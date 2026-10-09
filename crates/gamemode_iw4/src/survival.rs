use crate::phase::Team;

pub const GAMETYPE_TOKEN: &str = "survival";

pub const DISPLAY_NAME: &str = "SURVIVAL";

/// Survival ends when the players fall, not on a score: 0 is "no limit", as
/// for Demolition.
pub const SCORE_LIMIT: i32 = 0;

/// Waves run until the players fall: 0 is "no limit".
pub const TIME_LIMIT_MS: u32 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayingTeam {
    Axis,
    Allies,
}

impl PlayingTeam {
    fn parse(name: &str) -> Option<Self> {
        match name.trim().as_bytes() {
            b"axis" => Some(Self::Axis),
            b"allies" => Some(Self::Allies),
            _ => None,
        }
    }

    /// The name `sessionteam` and the team menu use.
    pub const fn script_name(self) -> &'static str {
        match self {
            Self::Axis => "axis",
            Self::Allies => "allies",
        }
    }

    pub const fn team(self) -> Team {
        match self {
            Self::Axis => Team::Axis,
            Self::Allies => Team::Allies,
        }
    }
}

/// The sides survival is played on: the players on one team, every wave on
/// the other. The names come from the mode's settings; this only holds them
/// to two opposing playing teams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TeamRules {
    pub human: PlayingTeam,
    pub enemy: PlayingTeam,
}

impl TeamRules {
    pub fn parse(human: &str, enemy: &str) -> Result<Self, &'static str> {
        let human =
            PlayingTeam::parse(human).ok_or("the human team is neither `allies` nor `axis`")?;
        let enemy =
            PlayingTeam::parse(enemy).ok_or("the enemy team is neither `allies` nor `axis`")?;
        if human == enemy {
            return Err("the human and enemy teams are the same team");
        }
        Ok(Self { human, enemy })
    }
}
