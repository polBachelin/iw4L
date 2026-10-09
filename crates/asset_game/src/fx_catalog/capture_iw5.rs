//! MW3's FxEffectDef and FxElemDef extend MW2's rather than rearrange them:
//! the effect header adds the occlusion-query fields before `elemDefs`, and
//! the elem appends one word after `fadeInfo`. Every field the catalog reads
//! sits at the offset it has in MW2, so the owned effect is the same shape and
//! the views in `fx_iw4` read it unchanged; only the zone stream differs.
//! `header_raw` and each elem's `raw` keep MW3's bytes as authored, longer
//! than MW2's; nothing reads past the fields the two share.

use std::collections::HashMap;

use fastfile_iw5::size as sz5;
use fastfile_iw5::{FxEffectDefGeometry, Iw5WireFormat, Ptr, ZonePtr, ZoneStream};
use fx_iw4::{FX_EFFECT_DEF_SIZE, FxEffectDefView, FxElemDefView, effect_def_view, elem_def_view};

use super::{
    FxCatalog, FxElemMaterial, OwnedFxEffectDef, OwnedFxElemDef, OwnedFxSparkFountainDef,
    OwnedFxTrailDef, OwnedFxVisual, capture_named_child, elem_type, mark_pair, model_visual,
    parse_spark_fountain_def, parse_trail_def, parse_vel_graph_channel, runner_visual,
    sound_visual, trail_vert_count,
};
use crate::asset_graph::AssetEdgeFromPtrs;
use crate::graph_support::{AuthoredRef, iw5_link_ptr as link_ptr};
use asset_iw4::size as sz;
use asset_material::MaterialCatalog;

// The sub-structures the elem points at keep MW2's sizes; the parsers shared
// with the MW2 capture rely on it.
const _: () = {
    assert!(sz5::FX_ELEM_VEL_STATE_SAMPLE == sz::FX_ELEM_VEL_STATE_SAMPLE);
    assert!(sz5::FX_ELEM_VIS_STATE_SAMPLE == sz::FX_ELEM_VIS_STATE_SAMPLE);
    assert!(sz5::FX_ELEM_MARK_VISUALS == sz::FX_ELEM_MARK_VISUALS);
    assert!(sz5::FX_ELEM_VISUALS == sz::FX_ELEM_VISUALS);
    assert!(sz5::FX_TRAIL_DEF == sz::FX_TRAIL_DEF);
    assert!(sz5::FX_TRAIL_VERTEX == sz::FX_TRAIL_VERTEX);
    assert!(sz5::FX_SPARK_FOUNTAIN_DEF == fx_iw4::FX_SPARK_FOUNTAIN_DEF_SIZE);
    assert!(sz5::FX_EFFECT_DEF >= FX_EFFECT_DEF_SIZE);
    assert!(sz5::FX_ELEM_DEF >= fx_iw4::FX_ELEM_DEF_STRIDE);
};

impl FxCatalog {
    /// `xmodel_names` maps an XModel slot to its name, as the walk recorded it.
    pub fn capture_iw5(
        &mut self,
        s: &ZoneStream<'_>,
        geometry: FxEffectDefGeometry,
        materials: &MaterialCatalog,
        xmodel_names: &HashMap<Ptr, Ptr>,
    ) {
        let Some(name) = geometry
            .name
            .and_then(|ptr| s.cstr(ptr).ok())
            .filter(|name| !name.is_empty())
        else {
            self.capture_gaps += 1;
            return;
        };
        let Ok(header_raw) = s.slice_at(geometry.header, 0, s.layout(sz5::FX_EFFECT_DEF, 64))
        else {
            self.capture_gaps += 1;
            return;
        };
        let view = if s.wire_format() == Iw5WireFormat::X64 {
            effect_view_x64(s, geometry)
        } else {
            effect_def_view(header_raw)
        };
        let Some(view) = view.filter(|view| {
            view.looping_count == geometry.looping_count
                && view.one_shot_count == geometry.one_shot_count
                && view.emission_count == geometry.emission_count
        }) else {
            self.capture_gaps += 1;
            return;
        };

        let mut elems = Vec::with_capacity(geometry.elem_def_count);
        if let Some(arr) = geometry.elem_defs {
            for i in 0..geometry.elem_def_count {
                let elem_ptr = arr.at(i * s.layout(sz5::FX_ELEM_DEF, 288));
                match capture_elem_iw5(s, elem_ptr, materials, xmodel_names) {
                    Some(elem) => elems.push(elem),
                    None => {
                        self.capture_gaps += 1;
                        return;
                    }
                }
            }
        } else if geometry.elem_def_count > 0 {
            self.capture_gaps += 1;
            return;
        }

        self.last_captured = Some(name.to_owned());
        let namespace = self
            .capture_ns
            .expect("asset capture requires an explicit family");
        self.insert_owned(OwnedFxEffectDef {
            namespace,
            name: name.to_owned(),
            view,
            header_raw: header_raw.to_vec(),
            elems,
        });
    }
}

fn effect_view_x64(s: &ZoneStream<'_>, geometry: FxEffectDefGeometry) -> Option<FxEffectDefView> {
    Some(FxEffectDefView {
        flags: s.i32_at(geometry.header, 8).ok()?,
        msec_looping_life: s.i32_at(geometry.header, 16).ok()?,
        looping_count: geometry.looping_count,
        one_shot_count: geometry.one_shot_count,
        emission_count: geometry.emission_count,
    })
}

fn capture_elem_iw5(
    s: &ZoneStream<'_>,
    p: Ptr,
    materials: &MaterialCatalog,
    xmodel_names: &HashMap<Ptr, Ptr>,
) -> Option<OwnedFxElemDef> {
    let raw = s
        .slice_at(p, 0, s.layout(sz5::FX_ELEM_DEF, 288))
        .ok()?
        .to_vec();
    let view = if s.wire_format() == Iw5WireFormat::X64 {
        fx_iw4::elem_def_view_x64(&raw)?
    } else {
        elem_def_view(&raw)?
    };
    let vel_count = view.vel_interval_count as usize + 1;
    let vis_count = view.vis_state_interval_count as usize + 1;
    let vel_samples = copy_ptr_array_iw5(
        s,
        p,
        s.layout(sz5::FX_ELEM_VEL_SAMPLES_OFF, 184),
        vel_count,
        sz5::FX_ELEM_VEL_STATE_SAMPLE,
    )?;
    let vel_graph_local = parse_vel_graph_channel(&vel_samples, vel_count, false);
    let vel_graph_world = parse_vel_graph_channel(&vel_samples, vel_count, true);
    let vis_samples = copy_ptr_array_iw5(
        s,
        p,
        s.layout(sz5::FX_ELEM_VIS_SAMPLES_OFF, 192),
        vis_count,
        sz5::FX_ELEM_VIS_STATE_SAMPLE,
    )?;
    let visuals = capture_visuals_iw5(s, p, &view, materials, xmodel_names);
    let (effect_on_impact, effect_on_impact_hint) = capture_named_child(read_name_field_iw5(
        s,
        p,
        s.layout(sz5::FX_ELEM_EFFECT_ON_IMPACT_OFF, 232),
    ));
    let (effect_on_death, effect_on_death_hint) = capture_named_child(read_name_field_iw5(
        s,
        p,
        s.layout(sz5::FX_ELEM_EFFECT_ON_DEATH_OFF, 240),
    ));
    let (effect_emitted, effect_emitted_hint) = capture_named_child(read_name_field_iw5(
        s,
        p,
        s.layout(sz5::FX_ELEM_EFFECT_EMITTED_OFF, 248),
    ));
    let extended = s.layout(sz5::FX_ELEM_EXTENDED_OFF, 272);
    let has_extended = !matches!(s.ptr_at(p, extended).ok()?, ZonePtr::Null);
    let trail_def = if view.elem_type == elem_type::TRAIL {
        capture_trail_def_iw5(s, p, extended)
    } else {
        None
    };
    let spark_fountain_def = if view.elem_type == elem_type::SPARK_FOUNTAIN {
        capture_spark_fountain_def_iw5(s, p, extended)
    } else {
        None
    };
    Some(OwnedFxElemDef {
        view,
        raw,
        vel_samples,
        vel_graph_local,
        vel_graph_world,
        vis_samples,
        visuals,
        effect_on_impact,
        effect_on_impact_hint,
        effect_on_death,
        effect_on_death_hint,
        effect_emitted,
        effect_emitted_hint,
        has_extended,
        trail_def,
        spark_fountain_def,
    })
}

fn extended_body(s: &ZoneStream<'_>, elem: Ptr, extended: usize) -> Option<Ptr> {
    match s.ptr_at(elem, extended).ok()? {
        ZonePtr::Offset(body) => Some(s.resolve_alias(body)),
        _ => None,
    }
}

fn capture_trail_def_iw5(
    s: &ZoneStream<'_>,
    elem: Ptr,
    extended: usize,
) -> Option<OwnedFxTrailDef> {
    let trail = extended_body(s, elem, extended)?;
    let header = s.slice_at(trail, 0, s.layout(sz5::FX_TRAIL_DEF, 48)).ok()?;
    let vert_count = trail_vert_count(header)?;
    let ind_count = s
        .i32_at(trail, s.layout(sz5::FX_TRAIL_IND_COUNT_OFF, 32))
        .ok()?
        .max(0) as usize;
    let vert_bytes = copy_ptr_array_iw5(
        s,
        trail,
        sz5::FX_TRAIL_VERTS_OFF,
        vert_count,
        sz5::FX_TRAIL_VERTEX,
    )?;
    let ind_bytes =
        copy_ptr_array_iw5(s, trail, s.layout(sz5::FX_TRAIL_INDS_OFF, 40), ind_count, 2)?;
    parse_trail_def(header, &vert_bytes, &ind_bytes)
}

fn capture_spark_fountain_def_iw5(
    s: &ZoneStream<'_>,
    elem: Ptr,
    extended: usize,
) -> Option<OwnedFxSparkFountainDef> {
    let def = extended_body(s, elem, extended)?;
    parse_spark_fountain_def(s.slice_at(def, 0, sz5::FX_SPARK_FOUNTAIN_DEF).ok()?)
}

/// The visual array of an elem with more than one visual, or `None` when the
/// zone did not leave it in the image.
fn visual_array(s: &ZoneStream<'_>, vis: Ptr) -> Option<Ptr> {
    match s.ptr_at(vis, 0).ok()? {
        ZonePtr::Offset(arr) => Some(s.resolve_alias(arr)),
        _ => None,
    }
}

fn capture_visuals_iw5(
    s: &ZoneStream<'_>,
    p: Ptr,
    view: &FxElemDefView,
    materials: &MaterialCatalog,
    xmodel_names: &HashMap<Ptr, Ptr>,
) -> Vec<OwnedFxVisual> {
    let t = view.elem_type;
    let visual_count = view.visual_count as usize;
    let vis = p.at(s.layout(sz5::FX_ELEM_VISUALS_OFF, 200));
    let visual_stride = s.layout(sz5::FX_ELEM_VISUALS, 8);

    if matches!(t, elem_type::OMNI_LIGHT | elem_type::SPOT_LIGHT) || visual_count == 0 {
        return vec![OwnedFxVisual::None];
    }
    if t == elem_type::DECAL {
        let Some(arr) = visual_array(s, vis) else {
            return (0..visual_count.max(1))
                .map(|_| OwnedFxVisual::UnresolvedMaterial)
                .collect();
        };
        return (0..visual_count.max(1))
            .map(|i| {
                let m = arr.at(i * s.layout(sz5::FX_ELEM_MARK_VISUALS, 16));
                mark_pair(
                    materials.capture_ns(),
                    resolve_material_visual_iw5(s, materials, m.at(0)),
                    resolve_material_visual_iw5(s, materials, m.at(s.layout(4, 8))),
                )
            })
            .collect();
    }
    let named = fastfile_iw5::size::fx_elem::is_sprite(t)
        || matches!(t, elem_type::MODEL | elem_type::RUNNER | elem_type::SOUND);
    if !named {
        return vec![OwnedFxVisual::None];
    }
    // One visual is stored in place; several sit in an array the field points at.
    let slots: Vec<Ptr> = if visual_count > 1 {
        match visual_array(s, vis) {
            Some(arr) => (0..visual_count)
                .map(|i| arr.at(i * visual_stride))
                .collect(),
            None => {
                return (0..visual_count)
                    .map(|_| unarrayed_visual(s, vis, t, materials))
                    .collect();
            }
        }
    } else {
        vec![vis]
    };
    let capture = |slot: Ptr| {
        if fastfile_iw5::size::fx_elem::is_sprite(t) {
            resolve_material_visual_iw5(s, materials, slot)
        } else if t == elem_type::MODEL {
            model_visual(read_xmodel_name_iw5(s, slot, xmodel_names))
        } else if t == elem_type::RUNNER {
            runner_visual(read_name_field_iw5(s, slot, 0))
        } else {
            sound_visual(read_name_field_iw5(s, slot, 0))
        }
    };
    slots.into_iter().map(capture).collect()
}

/// What an elem with several visuals holds when its visual array is missing:
/// a sprite keeps the reason as its hint, the other kinds have nothing to name.
fn unarrayed_visual(
    s: &ZoneStream<'_>,
    vis: Ptr,
    t: u8,
    materials: &MaterialCatalog,
) -> OwnedFxVisual {
    if !fastfile_iw5::size::fx_elem::is_sprite(t) {
        return OwnedFxVisual::None;
    }
    let kind = match s.ptr_at(vis, 0) {
        Ok(ZonePtr::Following) => "following",
        Ok(ZonePtr::Insert) => "insert",
        Ok(ZonePtr::Null) => "null",
        Ok(ZonePtr::Offset(_)) | Err(_) => "read_err",
    };
    OwnedFxVisual::Material {
        material: FxElemMaterial::unresolved_for(Some(link_ptr(vis)), None),
        hint: Some(kind.to_string()),
        material_namespace: materials.capture_ns(),
        authored: AuthoredRef::from_ptrs(Some(link_ptr(vis)), None),
    }
}

fn resolve_material_visual_iw5(
    s: &ZoneStream<'_>,
    materials: &MaterialCatalog,
    slot: Ptr,
) -> OwnedFxVisual {
    let alias = match s.ptr_at(slot, 0) {
        Ok(ZonePtr::Offset(target)) => Some(link_ptr(s.resolve_alias(target))),
        _ => None,
    };
    let slot = link_ptr(slot);
    let index = materials
        .material_index(slot)
        .or_else(|| alias.and_then(|target| materials.material_index(target)));
    let name = index
        .and_then(|i| materials.materials.get(i.get()).map(|m| m.name.to_string()))
        .filter(|n| !n.is_empty());

    OwnedFxVisual::Material {
        material: FxElemMaterial::unresolved_for(Some(slot), alias),
        hint: name,
        material_namespace: materials.capture_ns(),
        authored: AuthoredRef::from_ptrs(Some(slot), alias),
    }
}

fn read_xmodel_name_iw5(
    s: &ZoneStream<'_>,
    slot: Ptr,
    xmodel_names: &HashMap<Ptr, Ptr>,
) -> Option<String> {
    if let Some(name) = xmodel_names.get(&slot).copied() {
        return s.cstr(name).ok().map(str::to_owned);
    }
    let ZonePtr::Offset(body) = s.ptr_at(slot, 0).ok()? else {
        return None;
    };
    let body = s.resolve_alias(body);
    if let Some(name) = xmodel_names.get(&body).copied() {
        return s.cstr(name).ok().map(str::to_owned);
    }
    match s.ptr_at(body, 0).ok()? {
        ZonePtr::Offset(name) => s.cstr(s.resolve_alias(name)).ok().map(str::to_owned),
        _ => s.cstr(body).ok().map(str::to_owned),
    }
}

fn read_name_field_iw5(s: &ZoneStream<'_>, p: Ptr, field: usize) -> String {
    match s.ptr_at(p, field) {
        Ok(ZonePtr::Offset(name)) => s.cstr(s.resolve_alias(name)).ok().unwrap_or("").to_owned(),
        _ => String::new(),
    }
}

/// The `count` records the field points at. A null field is an array the
/// zone authored empty (MW3 leaves the visual samples of an elem that draws
/// nothing null); `None` is an array the walk did not leave in the image,
/// which makes the owner a capture gap.
fn copy_ptr_array_iw5(
    s: &ZoneStream<'_>,
    parent: Ptr,
    field: usize,
    count: usize,
    stride: usize,
) -> Option<Vec<u8>> {
    if count == 0 {
        return Some(Vec::new());
    }
    match s.ptr_at(parent, field).ok()? {
        ZonePtr::Null => Some(Vec::new()),
        ZonePtr::Offset(arr) => s
            .slice_at(s.resolve_alias(arr), 0, count.saturating_mul(stride))
            .ok()
            .map(<[u8]>::to_vec),
        ZonePtr::Following | ZonePtr::Insert => None,
    }
}
