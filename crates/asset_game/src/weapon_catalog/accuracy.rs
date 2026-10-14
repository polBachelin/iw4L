use std::sync::Arc;

use super::capture_iw5::leftover_xstring_at_iw5;

/// One accuracy graph: knots of (distance fraction, accuracy), abscissae
/// non-decreasing. A single knot is a constant graph; repeated abscissae are a
/// step.
#[derive(Clone, Debug, PartialEq)]
pub struct WeaponAccuracyGraph {
    pub name: String,
    pub knots: Vec<[f32; 2]>,
}

/// The accuracy graphs an AI wielder reads: against other AI, and against
/// players. A channel with no knots is `None`, even when it names a graph
/// (`iw5_riotshield_so` names `pistol.accu` against AI with no knots).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeaponAccuracyGraphs {
    pub vs_ai: Option<WeaponAccuracyGraph>,
    pub vs_player: Option<WeaponAccuracyGraph>,
}

/// `Ok(None)` when the weapon carries no graph at all (every IW4, T5 and T6
/// weapon); `Err` names why a carried graph was refused.
pub type WeaponAccuracy = Result<Option<Arc<WeaponAccuracyGraphs>>, String>;

const CHANNELS: [&str; 2] = ["vs_ai", "vs_player"];

/// The knots come from the complete def, the copy the def's own arrays are
/// sized from; the names come from the def body.
pub(super) fn capture_iw5_accuracy(
    stream: &fastfile_iw5::ZoneStream<'_>,
    geometry: &fastfile_iw5::WeaponGeometry,
) -> WeaponAccuracy {
    let names = [(1552, 1968), (1556, 1976)].map(|(x86, x64)| {
        geometry
            .weap_def
            .and_then(|def| leftover_xstring_at_iw5(stream, def, x86, x64))
    });
    let mut graphs = [None, None];
    for (channel, (graph, name)) in graphs.iter_mut().zip(names).enumerate() {
        let label = CHANNELS[channel];
        let count = usize::from(geometry.accuracy_graph_knot_counts[channel]);
        let knots = match (count, geometry.accuracy_graph_knots[channel]) {
            (0, _) => Vec::new(),
            (_, None) => return Err(format!("{label} graph has {count} knots and no array")),
            (_, Some(array)) => (0..count)
                .map(|i| {
                    Some([
                        stream.f32_at(array, i * 8).ok()?,
                        stream.f32_at(array, i * 8 + 4).ok()?,
                    ])
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| format!("{label} graph knots run past the zone"))?,
        };
        *graph = validate_graph(label, name, knots)?;
    }
    let [vs_ai, vs_player] = graphs;
    if vs_ai.is_none() && vs_player.is_none() {
        return Ok(None);
    }
    Ok(Some(Arc::new(WeaponAccuracyGraphs { vs_ai, vs_player })))
}

fn validate_graph(
    label: &str,
    name: Option<String>,
    knots: Vec<[f32; 2]>,
) -> Result<Option<WeaponAccuracyGraph>, String> {
    if knots.is_empty() {
        return Ok(None);
    }
    let Some(name) = name else {
        return Err(format!("{label} graph has knots and no name"));
    };
    if let Some(i) = knots
        .iter()
        .position(|k| !k[0].is_finite() || !k[1].is_finite())
    {
        return Err(format!("{label} graph {name} knot {i} is not finite"));
    }
    if let Some(i) = knots.windows(2).position(|pair| pair[1][0] < pair[0][0]) {
        return Err(format!(
            "{label} graph {name} distance decreases at knot {}",
            i + 1
        ));
    }
    Ok(Some(WeaponAccuracyGraph { name, knots }))
}
