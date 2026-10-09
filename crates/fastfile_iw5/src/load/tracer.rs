use super::{AssetLinkSink, follow_name, load_asset_at_observed, material_name_at};
use crate::asset_type::AssetType;
use crate::size as sz;
use crate::zone::{Result, TracerDefGeometry, XFILE_BLOCK_VIRTUAL, ZonePtr, ZoneStream};

pub(super) fn load_tracer(s: &mut ZoneStream<'_>, links: &mut dyn AssetLinkSink) -> Result<()> {
    let p = s.alloc_load(4, s.layout(sz::TRACER_DEF, 120))?;
    s.push(XFILE_BLOCK_VIRTUAL)?;
    follow_name(s, p, 0)?;
    let material_slot = p.at(s.layout(sz::TRACER_DEF_MATERIAL_OFF, 8));
    let material_fresh = load_asset_at_observed(s, AssetType::Material, material_slot, links)?;
    let name = match s.ptr_at(p, 0)? {
        ZonePtr::Offset(q) => Some(s.resolve_alias(q)),
        _ => None,
    };
    let material_name = if material_fresh {
        s.latest_material().and_then(|g| g.name)
    } else {
        material_name_at(s, links, material_slot)
    };
    let mut colors = [[0f32; 4]; sz::TRACER_COLOR_COUNT];
    for (i, row) in colors.iter_mut().enumerate() {
        for (c, cell) in row.iter_mut().enumerate() {
            *cell = s.f32_at(p, s.layout(sz::TRACER_DEF_COLORS_OFF, 40) + i * 16 + c * 4)?;
        }
    }
    links.capture_tracer(
        s,
        TracerDefGeometry {
            header: Some(p),
            name,
            material_slot: (s.ptr_at(material_slot, 0)? != ZonePtr::Null).then_some(material_slot),
            material_name,
            material_fresh,
            draw_interval: s.u32_at(p, s.layout(sz::TRACER_DEF_DRAW_INTERVAL_OFF, 16))?,
            speed: s.f32_at(p, s.layout(sz::TRACER_DEF_SPEED_OFF, 20))?,
            beam_length: s.f32_at(p, s.layout(sz::TRACER_DEF_BEAM_LENGTH_OFF, 24))?,
            beam_width: s.f32_at(p, s.layout(sz::TRACER_DEF_BEAM_WIDTH_OFF, 28))?,
            screw_radius: s.f32_at(p, s.layout(sz::TRACER_DEF_SCREW_RADIUS_OFF, 32))?,
            screw_dist: s.f32_at(p, s.layout(sz::TRACER_DEF_SCREW_DIST_OFF, 36))?,
            colors,
        },
    )?;
    s.pop()
}
