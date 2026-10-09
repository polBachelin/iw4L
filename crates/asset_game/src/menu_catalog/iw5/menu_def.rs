//! MW3 `menuDef_t` as the catalog reads it, after the zone walk has fixed up
//! every pointer below the header.
//!
//! MW3 keeps MW2's `windowDef_t`, `itemDef_s` field order (plus `hasFocus`
//! and minus `imageTrack`), expression entries and event handlers, but moves
//! the menu's own scalars, handlers and expressions into `menuData_t`. The
//! walk has already resolved every shared handler set and statement to an
//! offset, so this reads them where they are instead of replaying them.
//!
//! Expressions are written in the dump text the HUD evaluator parses
//! (`op N`, ints, `f:` floats, `s:` strings, `{ … }` functions). MW3 numbers
//! its operators differently from the evaluator, so each `op N` carries the
//! evaluator's number for the MW3 operator of the same name; an operator the
//! evaluator does not implement is written as `fnmiss`, so the expression
//! fails instead of running another operator.

use std::fmt::Write as _;

use fastfile_iw5::size as sz;
use fastfile_iw5::{Ptr, ZonePtr, ZoneStream};

use crate::localize::decode_localized_text;
use crate::menu_catalog::{
    ItemHandlers, MenuDef, MenuEvent, MenuHandlers, MenuItem, MenuRect, MenuSetLocalVar,
};

const EVENT_UNCONDITIONAL: u8 = 0;
const EVENT_IF: u8 = 1;
const EVENT_ELSE: u8 = 2;
const EVENT_SET_LOCAL_VAR_BOOL: u8 = 3;
const EVENT_SET_LOCAL_VAR_STRING: u8 = 6;

const EET_OPERAND: i32 = 1;
const VAL_INT: i32 = 0;
const VAL_FLOAT: i32 = 1;
const VAL_STRING: i32 = 2;
const VAL_FUNCTION: i32 = 3;

const ITEM_TYPE_LISTBOX: i32 = 6;
const ITEM_TYPE_MULTI: i32 = 12;

/// Handler sets and function statements nest; MW3's deepest HUD menus stay
/// far below this, so reaching it means a cycle in the walked data.
const MAX_NESTING: u32 = 32;

/// Names the walk resolved for asset pointers inside a menu.
pub(super) trait LinkedNames {
    fn material(&self, s: &ZoneStream<'_>, slot: Ptr) -> Option<String>;
    fn sound(&self, s: &ZoneStream<'_>, slot: Ptr) -> Option<String>;
}

#[derive(Debug)]
pub(super) enum MenuReadError {
    Zone(fastfile_iw5::ZoneError),
    Nesting,
    /// A non-null asset pointer the walk recorded no name for.
    UnnamedAsset {
        field: usize,
    },
}

impl std::fmt::Display for MenuReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Zone(error) => write!(f, "{error:?}"),
            Self::Nesting => write!(f, "handler sets or statements nest past {MAX_NESTING}"),
            Self::UnnamedAsset { field } => {
                write!(f, "asset pointer at +{field} has no name from the walk")
            }
        }
    }
}

impl From<fastfile_iw5::ZoneError> for MenuReadError {
    fn from(error: fastfile_iw5::ZoneError) -> Self {
        Self::Zone(error)
    }
}

type Read<T> = Result<T, MenuReadError>;

pub(super) fn read_menu_list(s: &ZoneStream<'_>, header: Ptr) -> Read<(String, i32)> {
    Ok((
        string_at(s, header, 0)?,
        s.i32_at(header, sz::MENU_LIST_COUNT_OFF)?,
    ))
}

pub(super) fn read_menu_def(
    s: &ZoneStream<'_>,
    header: Ptr,
    names: &impl LinkedNames,
) -> Read<MenuDef> {
    let window = header.at(sz::MENU_DEF_WINDOW_OFF);
    let mut def = MenuDef {
        name: string_at(s, window, sz::WINDOW_DEF_NAME_OFF)?,
        window_background: linked_name(s, window, sz::WINDOW_DEF_BACKGROUND_OFF, |slot| {
            names.material(s, slot)
        })?,
        static_flags: s.i32_at(window, sz::WINDOW_DEF_STATIC_FLAGS_OFF)?,
        rect: read_rect(s, window.at(sz::WINDOW_DEF_RECT_OFF))?,
        ..MenuDef::default()
    };
    if let Some(data) = deref(s, header, 0)? {
        read_menu_data(s, data, &mut def)?;
    }
    let item_count = s.i32_at(header, sz::MENU_DEF_ITEM_COUNT_OFF)?.max(0) as usize;
    if let Some(items) = deref(s, header, sz::MENU_DEF_ITEMS_OFF)? {
        for i in 0..item_count {
            if let Some(item) = deref(s, items, i * 4)? {
                def.items.push(read_item(s, item, names)?);
            }
        }
    }
    Ok(def)
}

fn read_menu_data(s: &ZoneStream<'_>, data: Ptr, def: &mut MenuDef) -> Read<()> {
    def.fullscreen = s.i32_at(data, sz::MENU_DATA_FULLSCREEN_OFF)?;
    def.sound_name = string_at(s, data, sz::MENU_DATA_SOUND_NAME_OFF)?;
    def.focus_color = Some(read_vec4(s, data, sz::MENU_DATA_FOCUS_COLOR_OFF)?);
    def.vis_exp = statement_at(s, data, sz::MENU_DATA_VISIBLE_EXP_OFF)?;
    for key in 0..sz::MENU_DATA_RECT_EXP_COUNT {
        let dump = statement_at(s, data, sz::MENU_DATA_EXPS_OFF + key * 4)?;
        if !dump.is_empty() {
            def.float_exp.push((key as u32, dump));
        }
    }
    if let Some(support) = deref(s, data, sz::MENU_DATA_EXPRESSION_DATA_OFF)? {
        def.set_expr_dvars(static_dvars(s, support)?);
    }

    let set = |index: usize| -> Read<Vec<MenuEvent>> {
        handler_set_at(s, data, sz::MENU_DATA_HANDLER_SETS_OFF + index * 4, 0)
    };
    // onFocusDueToClose (index 4) has no MW2 counterpart and no reader.
    def.handlers = MenuHandlers {
        open: set(0)?,
        close_request: set(1)?,
        close: set(2)?,
        esc: set(3)?,
    };
    def.on_open = flat_scripts(&def.handlers.open);
    def.on_close_request = flat_scripts(&def.handlers.close_request);
    def.on_close = flat_scripts(&def.handlers.close);
    def.on_esc = flat_scripts(&def.handlers.esc);
    def.on_open_local_vars = flat_local_vars(&def.handlers.open);
    Ok(())
}

fn read_item(s: &ZoneStream<'_>, p: Ptr, names: &impl LinkedNames) -> Read<MenuItem> {
    let item_type = s.i32_at(p, sz::ITEM_DEF_TYPE_OFF)?;
    let text = match deref(s, p, sz::ITEM_DEF_TEXT_OFF)? {
        Some(text) => decode_localized_text(s.cstr_bytes(text)?),
        None => String::new(),
    };
    let set = |index: usize| -> Read<Vec<MenuEvent>> {
        handler_set_at(s, p, sz::ITEM_DEF_HANDLER_SETS_OFF + index * 4, 0)
    };
    let mouse_enter_text = set(0)?;
    let mouse_exit_text = set(1)?;
    let mouse_enter = set(2)?;
    let mouse_exit = set(3)?;
    let mut action = set(4)?;
    let accept = set(5)?;
    let focus = set(6)?;
    let has_focus = set(7)?;
    let leave_focus = set(8)?;

    let mut choices = Vec::new();
    if let Some(type_data) = deref(s, p, sz::ITEM_DEF_TYPE_DATA_OFF)? {
        match item_type {
            ITEM_TYPE_LISTBOX => action.extend(handler_set_at(
                s,
                type_data,
                sz::LIST_BOX_DEF_ON_DOUBLE_CLICK_OFF,
                0,
            )?),
            ITEM_TYPE_MULTI => choices = multi_choices(s, type_data)?,
            _ => {}
        }
    }

    let mut float_exp = Vec::new();
    let float_count = s.i32_at(p, sz::ITEM_DEF_FLOAT_EXPRESSION_COUNT_OFF)?.max(0) as usize;
    if let Some(floats) = deref(s, p, sz::ITEM_DEF_FLOAT_EXPRESSIONS_OFF)? {
        for i in 0..float_count {
            let entry = floats.at(i * sz::ITEM_FLOAT_EXPRESSION);
            let dump = statement_at(s, entry, 4)?;
            if !dump.is_empty() {
                float_exp.push((s.u32_at(entry, 0)?, dump));
            }
        }
    }

    let mut flat_mouse_enter = flat_scripts(&mouse_enter_text);
    flat_mouse_enter.extend(flat_scripts(&mouse_enter));
    let mut flat_action = flat_scripts(&action);
    flat_action.extend(flat_scripts(&accept));
    // MW2's `*_ptr` encoding of the slot's pointer kind (0 null, 1 following,
    // 2 insert, 3 offset). After the walk, sub-structures read as offsets;
    // asset slots keep their wire kind.
    let present = |off: usize| -> Read<u8> {
        Ok(match s.ptr_at(p, off)? {
            ZonePtr::Null => 0,
            ZonePtr::Following => 1,
            ZonePtr::Insert => 2,
            ZonePtr::Offset(_) => 3,
        })
    };

    Ok(MenuItem {
        name: string_at(s, p, sz::WINDOW_DEF_NAME_OFF)?,
        text_key: text,
        item_type,
        style: s.i32_at(p, sz::WINDOW_DEF_STYLE_OFF)?,
        owner_draw: s.i32_at(p, sz::WINDOW_DEF_OWNER_DRAW_OFF)?,
        rect: read_rect(s, p.at(sz::WINDOW_DEF_RECT_OFF))?,
        fore_color: read_vec4(s, p, sz::WINDOW_DEF_FORE_COLOR_OFF)?,
        back_color: read_vec4(s, p, sz::WINDOW_DEF_BACK_COLOR_OFF)?,
        glow_color: read_vec4(s, p, sz::ITEM_DEF_GLOW_COLOR_OFF)?,
        text_scale: s.f32_at(p, sz::ITEM_DEF_TEXT_SCALE_OFF)?,
        font_enum: s.i32_at(p, sz::ITEM_DEF_FONT_ENUM_OFF)?,
        text_align_mode: s.i32_at(p, sz::ITEM_DEF_TEXT_ALIGN_MODE_OFF)?,
        text_align_x: s.f32_at(p, sz::ITEM_DEF_TEXT_ALIGN_X_OFF)?,
        text_align_y: s.f32_at(p, sz::ITEM_DEF_TEXT_ALIGN_Y_OFF)?,
        text_style: s.i32_at(p, sz::ITEM_DEF_TEXT_STYLE_OFF)?,
        game_msg_window_index: s.i32_at(p, sz::ITEM_DEF_GAME_MSG_WINDOW_INDEX_OFF)?,
        background: linked_name(s, p, sz::WINDOW_DEF_BACKGROUND_OFF, |slot| {
            names.material(s, slot)
        })?,
        focus_sound: linked_name(s, p, sz::ITEM_DEF_FOCUS_SOUND_OFF, |slot| {
            names.sound(s, slot)
        })?,
        dvar: string_at(s, p, sz::ITEM_DEF_DVAR_OFF)?,
        dvar_test: string_at(s, p, sz::ITEM_DEF_DVAR_TEST_OFF)?,
        enable_dvar: string_at(s, p, sz::ITEM_DEF_ENABLE_DVAR_OFF)?,
        local_var: string_at(s, p, sz::ITEM_DEF_LOCAL_VAR_OFF)?,
        choices,
        bg_ptr: present(sz::WINDOW_DEF_BACKGROUND_OFF)?,
        vis_ptr: present(sz::ITEM_DEF_VISIBLE_EXP_OFF)?,
        mat_ptr: present(sz::ITEM_DEF_MATERIAL_EXP_OFF)?,
        sound_ptr: present(sz::ITEM_DEF_FOCUS_SOUND_OFF)?,
        mouse_enter_ptr: present(sz::ITEM_DEF_HANDLER_SETS_OFF + 2 * 4)?,
        on_focus_ptr: present(sz::ITEM_DEF_HANDLER_SETS_OFF + 6 * 4)?,
        vis_exp: statement_at(s, p, sz::ITEM_DEF_VISIBLE_EXP_OFF)?,
        disabled_exp: statement_at(s, p, sz::ITEM_DEF_DISABLED_EXP_OFF)?,
        text_exp: statement_at(s, p, sz::ITEM_DEF_TEXT_EXP_OFF)?,
        material_exp: statement_at(s, p, sz::ITEM_DEF_MATERIAL_EXP_OFF)?,
        float_exp,
        static_flags: s.i32_at(p, sz::WINDOW_DEF_STATIC_FLAGS_OFF)?,
        dvar_flags: s.i32_at(p, sz::ITEM_DEF_DVAR_FLAGS_OFF)?,
        action: flat_action,
        mouse_enter: flat_mouse_enter,
        on_focus: flat_scripts(&focus),
        handlers: ItemHandlers {
            action,
            accept,
            mouse_enter: [mouse_enter_text, mouse_enter].concat(),
            mouse_exit: [mouse_exit_text, mouse_exit].concat(),
            focus,
            has_focus,
            leave_focus,
        },
        ..MenuItem::default()
    })
}

fn multi_choices(s: &ZoneStream<'_>, multi: Ptr) -> Read<Vec<(String, String)>> {
    let count = s
        .i32_at(multi, sz::MULTI_DEF_COUNT_OFF)?
        .clamp(0, sz::MULTI_DEF_CHOICES as i32) as usize;
    let string_values = s.i32_at(multi, sz::MULTI_DEF_STR_DEF_OFF)? != 0;
    let mut choices = Vec::with_capacity(count);
    for i in 0..count {
        let label = string_at(s, multi, sz::MULTI_DEF_LIST_OFF + i * 4)?;
        if label.is_empty() {
            continue;
        }
        let value = if string_values {
            string_at(s, multi, sz::MULTI_DEF_STRING_OFF + i * 4)?
        } else {
            s.f32_at(multi, sz::MULTI_DEF_FLOAT_OFF + i * 4)?
                .to_string()
        };
        choices.push((label, value));
    }
    Ok(choices)
}

fn handler_set_at(
    s: &ZoneStream<'_>,
    parent: Ptr,
    field: usize,
    depth: u32,
) -> Read<Vec<MenuEvent>> {
    let Some(set) = deref(s, parent, field)? else {
        return Ok(Vec::new());
    };
    if depth > MAX_NESTING {
        return Err(MenuReadError::Nesting);
    }
    let count = s.i32_at(set, 0)?.max(0) as usize;
    let Some(handlers) = deref(s, set, 4)? else {
        return Ok(Vec::new());
    };
    let mut events = Vec::with_capacity(count);
    for i in 0..count {
        let Some(handler) = deref(s, handlers, i * 4)? else {
            continue;
        };
        let kind = s.u8_at(handler, sz::MENU_EVENT_HANDLER_TYPE_OFF)?;
        match kind {
            EVENT_UNCONDITIONAL => {
                let script = string_at(s, handler, 0)?;
                if !script.is_empty() {
                    events.push(MenuEvent::Script(script));
                }
            }
            EVENT_IF => {
                if let Some(conditional) = deref(s, handler, 0)? {
                    events.push(MenuEvent::If {
                        condition: statement_at(
                            s,
                            conditional,
                            sz::CONDITIONAL_SCRIPT_EXPRESSION_OFF,
                        )?,
                        then: handler_set_at(s, conditional, 0, depth + 1)?,
                    });
                }
            }
            EVENT_ELSE => events.push(MenuEvent::Else(handler_set_at(s, handler, 0, depth + 1)?)),
            EVENT_SET_LOCAL_VAR_BOOL..=EVENT_SET_LOCAL_VAR_STRING => {
                if let Some(var) = deref(s, handler, 0)? {
                    events.push(MenuEvent::SetLocalVar {
                        kind: i32::from(kind),
                        name: string_at(s, var, 0)?,
                        expr: statement_at(s, var, sz::SET_LOCAL_VAR_DATA_EXPRESSION_OFF)?,
                    });
                }
            }
            // The walk loads no body for other event types either.
            _ => {}
        }
    }
    Ok(events)
}

/// Every unconditional script in the handler tree, branches included, in
/// order: the flat lists `MenuDef`/`MenuItem` keep beside the tree.
fn flat_scripts(events: &[MenuEvent]) -> Vec<String> {
    let mut out = Vec::new();
    for event in events {
        match event {
            MenuEvent::Script(script) => out.push(script.clone()),
            MenuEvent::If { then, .. } | MenuEvent::Else(then) => out.extend(flat_scripts(then)),
            MenuEvent::SetLocalVar { .. } => {}
        }
    }
    out
}

fn flat_local_vars(events: &[MenuEvent]) -> Vec<MenuSetLocalVar> {
    let mut out = Vec::new();
    for event in events {
        match event {
            MenuEvent::SetLocalVar { kind, name, expr } => out.push(MenuSetLocalVar {
                kind: *kind,
                name: name.clone(),
                expr: expr.clone(),
            }),
            MenuEvent::If { then, .. } | MenuEvent::Else(then) => out.extend(flat_local_vars(then)),
            MenuEvent::Script(_) => {}
        }
    }
    out
}

/// The `d <index> <name>` rows `MenuDef::static_dvar_name` reads.
fn static_dvars(s: &ZoneStream<'_>, support: Ptr) -> Read<String> {
    let list = support.at(sz::EXPRESSION_SUPPORTING_DATA_STATIC_DVARS_OFF);
    let count = s.i32_at(list, 0)?.max(0) as usize;
    let mut out = String::new();
    let Some(dvars) = deref(s, list, 4)? else {
        return Ok(out);
    };
    for index in 0..count {
        let Some(dvar) = deref(s, dvars, index * 4)? else {
            continue;
        };
        let name = string_at(s, dvar, sz::STATIC_DVAR_NAME_OFF)?;
        if !name.is_empty() {
            push_token(&mut out, "d");
            push_token(&mut out, &index.to_string());
            push_token(&mut out, &name);
        }
    }
    Ok(out)
}

fn statement_at(s: &ZoneStream<'_>, parent: Ptr, field: usize) -> Read<String> {
    let mut out = String::new();
    if let Some(statement) = deref(s, parent, field)? {
        dump_statement(s, statement, &mut out, 0)?;
    }
    Ok(out)
}

fn dump_statement(s: &ZoneStream<'_>, p: Ptr, out: &mut String, depth: u32) -> Read<()> {
    if depth > MAX_NESTING {
        return Err(MenuReadError::Nesting);
    }
    let count = s.i32_at(p, sz::STATEMENT_NUM_ENTRIES_OFF)?.max(0) as usize;
    let Some(entries) = deref(s, p, sz::STATEMENT_ENTRIES_OFF)? else {
        return Ok(());
    };
    for i in 0..count {
        let entry = entries.at(i * sz::EXPRESSION_ENTRY);
        let data = s.i32_at(entry, sz::EXPRESSION_ENTRY_DATA_OFF)?;
        if s.i32_at(entry, sz::EXPRESSION_ENTRY_TYPE_OFF)? != EET_OPERAND {
            match super::operators::evaluator_op(data) {
                Ok(op) => {
                    push_token(out, "op");
                    push_token(out, &op.to_string());
                }
                Err(missing) => {
                    push_token(out, "fnmiss");
                    push_token(out, &missing);
                }
            }
            continue;
        }
        let value = sz::EXPRESSION_ENTRY_OPERAND_VALUE_OFF;
        match data {
            VAL_INT => push_token(out, &s.i32_at(entry, value)?.to_string()),
            VAL_FLOAT => push_token(out, &format!("f:{:08x}", s.u32_at(entry, value)?)),
            VAL_STRING => {
                let mut token = String::from("s:");
                if let Some(text) = deref(s, entry, value)? {
                    for byte in s.cstr_bytes(text)? {
                        let _ = write!(token, "{byte:02x}");
                    }
                }
                push_token(out, &token);
            }
            VAL_FUNCTION => {
                push_token(out, "{");
                if let Some(body) = deref(s, entry, value)? {
                    dump_statement(s, body, out, depth + 1)?;
                }
                push_token(out, "}");
            }
            // An operand type the evaluator cannot read must fail the
            // expression, not drop out of it.
            other => {
                push_token(out, "fnmiss");
                push_token(out, &format!("operand-type-{other}"));
            }
        }
    }
    Ok(())
}

fn push_token(out: &mut String, token: &str) {
    if !out.is_empty() {
        out.push(' ');
    }
    out.push_str(token);
}

/// The name of the asset an asset-pointer field links. Asset slots keep
/// their wire value after the walk (the asset header is temp memory), so a
/// non-null slot is named through what the walk recorded for it.
fn linked_name(
    s: &ZoneStream<'_>,
    parent: Ptr,
    field: usize,
    lookup: impl FnOnce(Ptr) -> Option<String>,
) -> Read<String> {
    match s.ptr_at(parent, field)? {
        ZonePtr::Null => Ok(String::new()),
        _ => lookup(parent.at(field)).ok_or(MenuReadError::UnnamedAsset { field }),
    }
}

/// The fixed-up target of a pointer field, or `None` when it is null.
fn deref(s: &ZoneStream<'_>, parent: Ptr, field: usize) -> Read<Option<Ptr>> {
    Ok(match s.ptr_at(parent, field)? {
        ZonePtr::Offset(target) => Some(s.resolve_alias(target)),
        _ => None,
    })
}

fn string_at(s: &ZoneStream<'_>, parent: Ptr, field: usize) -> Read<String> {
    Ok(match deref(s, parent, field)? {
        Some(text) => s.cstr(text)?.to_owned(),
        None => String::new(),
    })
}

fn read_rect(s: &ZoneStream<'_>, p: Ptr) -> Read<MenuRect> {
    Ok(MenuRect {
        x: s.f32_at(p, 0)?,
        y: s.f32_at(p, 4)?,
        w: s.f32_at(p, 8)?,
        h: s.f32_at(p, 12)?,
        horz_align: s.u8_at(p, sz::RECT_DEF_HORZ_ALIGN_OFF)?,
        vert_align: s.u8_at(p, sz::RECT_DEF_VERT_ALIGN_OFF)?,
    })
}

fn read_vec4(s: &ZoneStream<'_>, p: Ptr, off: usize) -> Read<[f32; 4]> {
    Ok([
        s.f32_at(p, off)?,
        s.f32_at(p, off + 4)?,
        s.f32_at(p, off + 8)?,
        s.f32_at(p, off + 12)?,
    ])
}
