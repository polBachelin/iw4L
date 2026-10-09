use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use asset_core::AssetNamespace;
use asset_material::{MaterialCatalog, TechsetKey, TechsetResolve};
use fastfile_iw5::{
    AssetLinkSink, AssetSink, AssetType, Iw5WireFormat, Ptr, ScriptStrings, ZonePtr, ZoneStream,
};

use crate::localize::decode_localized_text;

use super::{
    CapturedStringTable, CapturedZoneImage, HudMaterialCapture, HudMaterialTextureBinding,
    HudTechniqueSlots, MenuCatalog, UiImageSet, insert_font, insert_rawfile, record_hud_material,
};

mod menu_def;
mod operators;

const _: () = assert!(fastfile_iw5::size::GLYPH == asset_iw4::size::GLYPH);

pub(super) fn load_menu_catalog(
    path: &Path,
    image: &asset_transport::ZoneImage,
    games: Option<&Path>,
    russian_fonts: bool,
) -> Result<MenuCatalog, String> {
    let header = image.iw5_header().map_err(|e| e.to_string())?;
    let mut memory = crate::Iw5ZoneMemory::for_header(&header);
    let mut stream = memory.stream(&image.bytes).map_err(|e| e.to_string())?;
    let mut materials = MaterialCatalog::default();
    materials.set_capture_ns(AssetNamespace::Iw5);
    materials.prepare_for_next_zone();
    let mut sink = Iw5MenuSink {
        catalog: MenuCatalog::default(),
        russian_fonts,
        materials,
        material_names: HashMap::new(),
        sound_names: HashMap::new(),
        pending_sound_name: None,
        menu_gaps: Vec::new(),
        loading_asset: None,
    };
    fastfile_iw5::load_zone(&mut stream, &mut sink).map_err(|error| match sink.loading_asset {
        Some((index, ty, slot)) => format!("asset #{index} {ty:?} at {slot:?}: {error:?}"),
        None => format!("asset table: {error:?}"),
    })?;
    Ok(sink.finish_and_take(path, games))
}

/// Reads a loaded IW5 `StringTable`. A table with any cell, count or name the
/// zone does not hold is refused whole: GSC `tableLookup` and the class
/// catalog read these cells as data, so an unreadable one is never stood in
/// for by an empty string. The zones write an empty cell as a pointer to "",
/// never as a null pointer. Cell text is the zone's code-page bytes, decoded
/// like menu item text (`sp/survival_waves.csv` carries a CP1252 ellipsis).
pub fn capture_iw5_string_table(
    s: &ZoneStream<'_>,
    header: Ptr,
) -> Result<CapturedStringTable, String> {
    let name = match s.ptr_at(header, 0) {
        Ok(ZonePtr::Offset(p)) => s
            .cstr(s.resolve_alias(p))
            .map_err(|error| format!("string table at {header:?}: unreadable name: {error}"))?
            .to_owned(),
        Ok(other) => return Err(format!("string table at {header:?}: no name ({other:?})")),
        Err(error) => {
            return Err(format!(
                "string table at {header:?}: unreadable name: {error}"
            ));
        }
    };
    if name.is_empty() {
        return Err(format!("string table at {header:?}: empty name"));
    }
    let count = |what: &str, off: usize| match s.i32_at(header, off) {
        Ok(n) => usize::try_from(n).map_err(|_| format!("{name}: negative {what} count {n}")),
        Err(error) => Err(format!("{name}: unreadable {what} count: {error}")),
    };
    let columns = count("column", s.layout(4, 8))?;
    let rows = count("row", s.layout(8, 12))?;
    let cells_n = columns
        .checked_mul(rows)
        .ok_or_else(|| format!("{name}: {columns}x{rows} cells overflow"))?;
    let arr = match s.ptr_at(header, s.layout(12, 16)) {
        Ok(ZonePtr::Offset(p)) => Some(s.resolve_alias(p)),
        _ => None,
    };
    let mut cells = Vec::with_capacity(cells_n);
    let mut unreadable = 0usize;
    if let Some(arr) = arr {
        let cell_sz = s.layout(fastfile_iw5::size::STRING_TABLE_CELL, 16);
        for i in 0..cells_n {
            let value = match s.ptr_at(arr.at(i * cell_sz), 0) {
                Ok(ZonePtr::Offset(p)) => s.cstr_bytes(s.resolve_alias(p)).ok(),
                _ => None,
            };
            match value {
                Some(value) => cells.push(decode_localized_text(value)),
                None => unreadable += 1,
            }
        }
    } else {
        unreadable = cells_n;
    }
    if unreadable != 0 {
        return Err(format!("{name}: {unreadable} unreadable cells"));
    }
    Ok(CapturedStringTable {
        name,
        columns,
        rows,
        cells,
    })
}

struct Iw5MenuSink {
    catalog: MenuCatalog,
    russian_fonts: bool,
    materials: MaterialCatalog,
    material_names: HashMap<Ptr, Ptr>,
    /// Sound-alias-list slot -> its name, for item focus sounds.
    sound_names: HashMap<Ptr, Ptr>,
    pending_sound_name: Option<Ptr>,
    /// Menus the catalog could not read, with the reason.
    menu_gaps: Vec<String>,
    loading_asset: Option<(usize, AssetType, Ptr)>,
}

impl menu_def::LinkedNames for Iw5MenuSink {
    fn material(&self, s: &ZoneStream<'_>, slot: Ptr) -> Option<String> {
        let name = self.material_names.get(&slot)?;
        s.cstr(*name).ok().map(str::to_owned)
    }

    fn sound(&self, s: &ZoneStream<'_>, slot: Ptr) -> Option<String> {
        let name = self.sound_names.get(&slot)?;
        s.cstr(*name).ok().map(str::to_owned)
    }
}

impl Iw5MenuSink {
    fn finish_and_take(mut self, path: &Path, games: Option<&Path>) -> MenuCatalog {
        self.materials.promote_iw5_fallback_tables();
        let mut ui_images = UiImageSet {
            images: self
                .materials
                .images
                .iter()
                .map(|image| CapturedZoneImage {
                    name: image.name.as_str().to_owned(),
                    use_srgb_reads: image.use_srgb_reads,
                    width: image.width,
                    height: image.height,
                    format: image.format,
                    payload: Arc::clone(&image.payload),
                })
                .collect(),
            material_ts2d: HashMap::new(),
        };
        for material in &self.materials.materials {
            // A reference names a material another zone holds; the walk of
            // that zone records it.
            if material.name.is_reference() {
                continue;
            }
            let key = material.name.as_str().to_ascii_lowercase();
            if key.is_empty() {
                continue;
            }
            let Some(texture_atlas) = material.texture_atlas else {
                diag::warn!(
                    Zone,
                    "menu catalog: IW5 material `{key}` has no texture atlas counts"
                );
                continue;
            };
            let technique = match self.materials.resolve_technique_set(TechsetKey::new(
                AssetNamespace::Iw5,
                material.technique_set.as_str(),
            )) {
                TechsetResolve::Hit { facts, .. } => {
                    facts.table.as_ref().map(|table| HudTechniqueSlots {
                        slots: table.slots,
                        scanned: table.scanned,
                        unlit_pass_count: table.pass_count_by_slot[asset_iw4::TECHNIQUE_UNLIT],
                    })
                }
                _ => None,
            };
            let textures = material
                .textures
                .iter()
                .map(|texture| {
                    (
                        HudMaterialTextureBinding {
                            name_hash: texture.name_hash,
                            name_start: texture.name_start,
                            name_end: texture.name_end,
                            sampler_state: texture.sampler_state,
                            semantic: texture.semantic,
                            image: None,
                        },
                        texture.image,
                    )
                })
                .collect();
            record_hud_material(
                &mut self.catalog,
                &mut ui_images,
                HudMaterialCapture {
                    key,
                    texture_atlas,
                    state_bits_entry: material.state_bits_entry,
                    state_rows: material.state_bits.clone(),
                    technique,
                    textures,
                },
            );
        }
        for gap in &self.menu_gaps {
            diag::warn!(Zone, "menu catalog: {}: {gap}", path.display());
        }
        if self.materials.capture_gaps != 0 {
            diag::info!(
                Zone,
                "menu catalog: {}: {} IW5 material/image capture gaps",
                path.display(),
                self.materials.capture_gaps
            );
        }
        let mut catalog = self.catalog;
        ui_images.install(&mut catalog, games);
        catalog
    }
}

impl AssetSink for Iw5MenuSink {
    fn set_script_strings(&mut self, _strings: ScriptStrings) {}

    fn load_asset(
        &mut self,
        s: &mut ZoneStream<'_>,
        index: usize,
        ty: AssetType,
        slot: Ptr,
    ) -> fastfile_iw5::Result<()> {
        self.loading_asset = Some((index, ty, slot));
        self.catalog.walked += 1;
        if ty == AssetType::Font {
            self.catalog.font_headers += 1;
        }
        fastfile_iw5::load_asset_at_observed(s, ty, slot, self)?;
        self.loading_asset = None;
        Ok(())
    }
}

impl AssetLinkSink for Iw5MenuSink {
    fn loaded(
        &mut self,
        s: &ZoneStream<'_>,
        ty: AssetType,
        slot: Ptr,
        insert_slot: Option<Ptr>,
    ) -> fastfile_iw5::Result<()> {
        self.materials.iw5_loaded(s, ty, slot, insert_slot);
        if ty == AssetType::Sound
            && let Some(name) = self.pending_sound_name.take()
        {
            self.sound_names.insert(slot, name);
            if let Some(insert_slot) = insert_slot {
                self.sound_names.insert(insert_slot, name);
            }
        }
        Ok(())
    }

    fn alias(&mut self, ty: AssetType, slot: Ptr, target: Ptr) -> fastfile_iw5::Result<()> {
        self.materials.iw5_alias(ty, slot, target);
        let names = match ty {
            AssetType::Material => &mut self.material_names,
            AssetType::Sound => &mut self.sound_names,
            _ => return Ok(()),
        };
        match names.get(&target).copied() {
            Some(name) => names.insert(slot, name),
            None => names.remove(&slot),
        };
        Ok(())
    }

    fn remember_material_name(&mut self, slot: Ptr, insert_slot: Option<Ptr>, name: Ptr) {
        self.material_names.insert(slot, name);
        if let Some(insert_slot) = insert_slot {
            self.material_names.insert(insert_slot, name);
        }
    }

    fn material_name_ptr(&self, slot: Ptr) -> Option<Ptr> {
        self.material_names.get(&slot).copied()
    }

    fn capture_sound(
        &mut self,
        s: &ZoneStream<'_>,
        list: Ptr,
        _count: usize,
        _head: Option<Ptr>,
    ) -> fastfile_iw5::Result<()> {
        self.pending_sound_name = match s.ptr_at(list, 0)? {
            ZonePtr::Offset(name) => Some(s.resolve_alias(name)),
            _ => None,
        };
        Ok(())
    }

    fn capture_menu_list(&mut self, s: &ZoneStream<'_>, header: Ptr) -> fastfile_iw5::Result<()> {
        match menu_def::read_menu_list(s, header) {
            Ok(list) => self.catalog.lists.push(list),
            Err(error) => self.menu_gaps.push(format!("menu list: {error}")),
        }
        Ok(())
    }

    fn capture_menu_def(&mut self, s: &ZoneStream<'_>, header: Ptr) -> fastfile_iw5::Result<()> {
        // The menu offsets in `fastfile_iw5::size` are the PC (x86) layout.
        if s.wire_format() == Iw5WireFormat::X64 {
            self.menu_gaps
                .push("menuDef_t: no x64 menu layout is read".to_owned());
            return Ok(());
        }
        match menu_def::read_menu_def(s, header, self) {
            Ok(def) if def.name.is_empty() => {
                self.menu_gaps.push("menuDef_t without a name".to_owned())
            }
            Ok(def) => {
                self.catalog.menus.insert(def.name.clone(), def);
            }
            Err(error) => self.menu_gaps.push(format!("menuDef_t: {error}")),
        }
        Ok(())
    }

    fn capture_font(
        &mut self,
        s: &ZoneStream<'_>,
        geometry: fastfile_iw5::FontDefGeometry,
    ) -> fastfile_iw5::Result<()> {
        let name_of = |ptr: Option<Ptr>| ptr.and_then(|ptr| s.cstr(ptr).ok()).unwrap_or("");
        let name = name_of(geometry.name);
        if name.is_empty() {
            return Ok(());
        }
        let glyphs = match geometry.glyphs {
            Some(glyphs) if geometry.glyph_count > 0 => s.slice_at(
                glyphs,
                0,
                geometry
                    .glyph_count
                    .saturating_mul(fastfile_iw5::size::GLYPH),
            )?,
            _ => &[],
        };
        insert_font(
            &mut self.catalog,
            &fastfile_iw4::FontCapture {
                name,
                pixel_height: geometry.pixel_height,
                material: name_of(geometry.material_name),
                glow_material: name_of(geometry.glow_material_name),
                glyphs,
            },
            self.russian_fonts,
        );
        Ok(())
    }

    fn capture_raw_file(
        &mut self,
        name: &str,
        data: &[u8],
        zlib_compressed: bool,
    ) -> fastfile_iw5::Result<()> {
        insert_rawfile(&mut self.catalog, name, data, zlib_compressed);
        Ok(())
    }

    fn capture_string_table(
        &mut self,
        s: &ZoneStream<'_>,
        header: Ptr,
    ) -> fastfile_iw5::Result<()> {
        match capture_iw5_string_table(s, header) {
            Ok(table) => {
                self.catalog.string_tables.insert(table.name.clone(), table);
            }
            Err(error) => self.menu_gaps.push(format!("string table: {error}")),
        }
        Ok(())
    }
}
