#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Command {
    Open(String),
    Close(String),
    Escape(String),
    SetFocus(Arg),
    FocusFirst,
    SetItemColor {
        item: String,
        field: String,
        color: [f32; 4],
    },
    Show(String),
    Hide(String),
    Play(String),
    ScriptMenuResponse(Arg),
    Exec(String),
    SetDvar(String, Arg),
    CopyDvar(String, String),
    ExecOnDvarIntValue {
        dvar: String,
        value: i32,
        command: String,
    },
    ExecOnDvarStringValue {
        dvar: String,
        value: String,
        command: String,
    },
    OpenForGameType(String),
    CloseForGameType(String),
    SetFocusByDvar(String),
    MenuOnDvar {
        open: bool,
        want_match: bool,
        dvar: String,
        value: String,
        menu: String,
    },
    /// `uiscript simulateKeyPress <key>`: the menu acts as if the key, a key
    /// code, were pressed.
    SimulateKeyPress(i32),
    /// A command whose inline call cannot be evaluated: it is not run.
    BadCall(String),
    Unhandled(String),
}

/// A command argument. MW3's menus pass some arguments as an inline call
/// (`"scriptmenuresponse" "localVarString" ( "purchase_item" )`): the command
/// takes the call's value when it runs, not the function's name.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Arg {
    Text(String),
    /// The call as an evaluator dump (`op <function> <arg> op 17 <arg> op 1`).
    Call(String),
}

struct Token {
    text: String,
    quoted: bool,
}

impl Token {
    fn is(&self, punctuation: &str) -> bool {
        !self.quoted && self.text == punctuation
    }
}

fn lex(script: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut chars = script.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() || c == ';' {
            chars.next();
            continue;
        }
        let mut text = String::new();
        let quoted = c == '"';
        if quoted {
            chars.next();
            for c in chars.by_ref() {
                if c == '"' {
                    break;
                }
                text.push(c);
            }
        } else {
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() || c == ';' || c == '"' {
                    break;
                }
                text.push(c);
                chars.next();
            }
        }
        out.push(Token { text, quoted });
    }
    out
}

pub(crate) fn tokens(script: &str) -> Vec<String> {
    lex(script).into_iter().map(|token| token.text).collect()
}

/// A token, or a function name and the parenthesized arguments after it
/// folded into one call.
enum Word {
    Text(String),
    Call {
        name: String,
        dump: Result<String, String>,
    },
}

impl Word {
    fn text(&self) -> String {
        match self {
            Word::Text(text) | Word::Call { name: text, .. } => text.clone(),
        }
    }
}

fn words(tokens: &[Token]) -> Vec<Word> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if tokens.get(i + 1).is_some_and(|t| t.is("(")) {
            let (dump, next) = call(tokens, i);
            out.push(Word::Call {
                name: tokens[i].text.clone(),
                dump,
            });
            i = next;
        } else {
            out.push(Word::Text(tokens[i].text.clone()));
            i += 1;
        }
    }
    out
}

/// The call whose function name is `tokens[at]`, as an evaluator dump, and
/// the index past its closing parenthesis. Arguments are literals or calls,
/// the only forms MW3's menu scripts write.
fn call(tokens: &[Token], at: usize) -> (Result<String, String>, usize) {
    let name = &tokens[at].text;
    let op = hud_iw4::op_by_name(&name.to_ascii_lowercase());
    let mut fault = None;
    let mut args = Vec::new();
    let mut i = at + 2;
    loop {
        let Some(token) = tokens.get(i) else {
            fault = fault.or_else(|| Some(format!("`{name} (` has no closing parenthesis")));
            break;
        };
        i += 1;
        if token.is(")") {
            break;
        }
        if token.is(",") {
            continue;
        }
        if token.is("(") {
            fault = fault.or_else(|| Some(format!("`{name}` takes an expression argument")));
            continue;
        }
        if tokens.get(i).is_some_and(|t| t.is("(")) {
            let (dump, next) = call(tokens, i - 1);
            match dump {
                Ok(dump) => args.push(dump),
                Err(error) => fault = fault.or(Some(error)),
            }
            i = next;
            continue;
        }
        args.push(literal(token));
    }
    let dump =
        match (op, fault) {
            (Some(op), None) => {
                let mut dump = format!("op {op}");
                for (index, arg) in args.iter().enumerate() {
                    if index > 0 {
                        dump.push_str(&format!(" op {}", hud_iw4::expr::OP_COMMA));
                    }
                    dump.push(' ');
                    dump.push_str(arg);
                }
                dump.push_str(&format!(" op {}", hud_iw4::expr::OP_RIGHTPAREN));
                Ok(dump)
            }
            (_, fault) => Err(fault
                .unwrap_or_else(|| format!("`{name}` is no function the evaluator implements"))),
        };
    (dump, i)
}

/// A literal argument as a dump operand: a quoted token is a string, a bare
/// number a number.
fn literal(token: &Token) -> String {
    if !token.quoted {
        if let Ok(int) = token.text.parse::<i32>() {
            return int.to_string();
        }
        if let Ok(float) = token.text.parse::<f32>() {
            return format!("f:{:08x}", float.to_bits());
        }
    }
    let mut out = String::from("s:");
    for byte in token.text.bytes() {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

pub(crate) fn parse(script: &str) -> Vec<Command> {
    let toks = words(&lex(script));
    let mut out = Vec::new();
    let mut i = 0;
    let arg = |i: usize| toks.get(i).map_or_else(String::new, Word::text);
    let value = |i: usize| match toks.get(i) {
        Some(Word::Call { dump: Ok(dump), .. }) => Arg::Call(dump.clone()),
        other => Arg::Text(other.map_or_else(String::new, Word::text)),
    };
    let float = |i: usize| arg(i).parse::<f32>().unwrap_or(0.0);
    while i < toks.len() {
        let name = toks[i].text().to_ascii_lowercase();
        if let Word::Call { .. } = toks[i] {
            out.push(Command::Unhandled(format!("{name} ( … )")));
            i += 1;
            continue;
        }
        let (command, used) = match name.as_str() {
            "open" | "ingameopen" => (Command::Open(arg(i + 1)), 2),
            // One local player per menu stack: closing for all of them is
            // closing it.
            "close" | "ingameclose" | "closeforallplayers" => (Command::Close(arg(i + 1)), 2),
            "escape" => (Command::Escape(arg(i + 1)), 2),
            "setfocus" => (Command::SetFocus(value(i + 1)), 2),
            "focusfirst" => (Command::FocusFirst, 1),
            "setitemcolor" => (
                Command::SetItemColor {
                    item: arg(i + 1),
                    field: arg(i + 2).to_ascii_lowercase(),
                    color: [float(i + 3), float(i + 4), float(i + 5), float(i + 6)],
                },
                7,
            ),
            "show" | "showmenu" => (Command::Show(arg(i + 1)), 2),
            "hide" | "hidemenu" => (Command::Hide(arg(i + 1)), 2),
            "play" => (Command::Play(arg(i + 1)), 2),
            "scriptmenuresponse" => (Command::ScriptMenuResponse(value(i + 1)), 2),
            "exec" | "execnow" => (Command::Exec(arg(i + 1)), 2),
            "copydvar" => (Command::CopyDvar(arg(i + 1), arg(i + 2)), 3),
            "setdvar" => (Command::SetDvar(arg(i + 1), value(i + 2)), 3),
            "execondvarintvalue" | "execnowondvarintvalue" => (
                Command::ExecOnDvarIntValue {
                    dvar: arg(i + 1),
                    value: arg(i + 2).parse().unwrap_or(0),
                    command: arg(i + 3),
                },
                4,
            ),
            "execondvarstringvalue" => (
                Command::ExecOnDvarStringValue {
                    dvar: arg(i + 1),
                    value: arg(i + 2),
                    command: arg(i + 3),
                },
                4,
            ),
            "setfocusbydvar" => (Command::SetFocusByDvar(arg(i + 1)), 2),
            "openforgametype" => (Command::OpenForGameType(arg(i + 1)), 2),
            "closeforgametype" => (Command::CloseForGameType(arg(i + 1)), 2),
            "uiscript" => {
                let script = arg(i + 1).to_ascii_lowercase();
                match script.as_str() {
                    "quit" => (Command::Exec("quit".into()), 2),
                    "openmenuondvar" | "openmenuondvarnot" | "closemenuondvar"
                    | "closemenuondvarnot" => (
                        Command::MenuOnDvar {
                            open: script.starts_with("open"),
                            want_match: !script.ends_with("not"),
                            dvar: arg(i + 2),
                            value: arg(i + 3),
                            menu: arg(i + 4),
                        },
                        5,
                    ),
                    "simulatekeypress" => match arg(i + 2).parse() {
                        Ok(key) => (Command::SimulateKeyPress(key), 3),
                        Err(_) => (
                            Command::Unhandled(format!("uiscript {script} {}", arg(i + 2))),
                            3,
                        ),
                    },
                    _ => (Command::Unhandled(format!("uiscript {script}")), 2),
                }
            }
            "fadein" | "fadeout" | "setbackground" | "playlooped" => {
                (Command::Unhandled(format!("{name} {}", arg(i + 1))), 2)
            }
            "setcolor" => (Command::Unhandled(name.clone()), 6),
            _ => (Command::Unhandled(name.clone()), 1),
        };
        // A call only `value` arguments evaluate: anywhere else the command
        // would run on the function's name.
        let call = toks[i + 1..(i + used).min(toks.len())]
            .iter()
            .enumerate()
            .find_map(|(at, word)| match word {
                Word::Call {
                    dump: Err(error), ..
                } => Some(error.clone()),
                Word::Call { name: function, .. }
                    if !matches!(
                        (&command, at),
                        (Command::SetFocus(_) | Command::ScriptMenuResponse(_), 0)
                            | (Command::SetDvar(..), 1)
                    ) =>
                {
                    Some(format!("argument {} is a call of `{function}`", at + 1))
                }
                _ => None,
            });
        match call {
            Some(problem) => out.push(Command::BadCall(format!("{name}: {problem}"))),
            None => out.push(command),
        }
        i += used;
    }
    out
}
