use super::{AssetLinkSink, always_array, follow_name, load_asset_at_observed, material_name_at};
use crate::size as sz;
use crate::zone::{
    FontDefGeometry, Result, XFILE_BLOCK_SCRIPT, XFILE_BLOCK_VIRTUAL, ZonePtr, ZoneStream,
};

fn linked_name<'a>(s: &'a ZoneStream<'_>, p: crate::zone::Ptr) -> &'a str {
    match s.ptr_at(p, 0).ok() {
        Some(ZonePtr::Offset(np)) => s.cstr(s.resolve_alias(np)).ok().unwrap_or(""),
        _ => "",
    }
}

pub(super) fn load_raw_file(s: &mut ZoneStream<'_>, links: &mut dyn AssetLinkSink) -> Result<()> {
    s.walk_stage = "raw_file";
    let p = s.alloc_load(4, s.layout(sz::RAW_FILE, 24))?;
    let compressed_len = s.i32_at(p, s.layout(4, 8))?;
    let len = s.i32_at(p, s.layout(8, 12))?;
    s.push(XFILE_BLOCK_VIRTUAL)?;
    follow_name(s, p, 0)?;
    let zlib_compressed = compressed_len > 0;
    let bytes = if zlib_compressed {
        compressed_len as usize
    } else if len >= 0 {
        (len as usize).saturating_add(1)
    } else {
        0
    };
    let data_ptr = s.plain_array(p, s.layout(12, 16), 1, 1, bytes)?;
    if let Some(px) = data_ptr {
        let raw = s.slice_at(px, 0, bytes).unwrap_or(&[]);
        let name = linked_name(s, p);
        if !name.is_empty() {
            links.capture_raw_file(name, raw, zlib_compressed)?;
        }
    }
    s.pop()
}

pub(super) fn load_string_table(
    s: &mut ZoneStream<'_>,
    links: &mut dyn AssetLinkSink,
) -> Result<()> {
    s.walk_stage = "string_table";
    let p = s.alloc_load(4, s.layout(sz::STRING_TABLE, 24))?;
    let columns = s.i32_at(p, s.layout(4, 8))?.max(0) as usize;
    let rows = s.i32_at(p, s.layout(8, 12))?.max(0) as usize;
    s.push(XFILE_BLOCK_VIRTUAL)?;
    follow_name(s, p, 0)?;
    let cells = columns.saturating_mul(rows);
    let cell = s.layout(sz::STRING_TABLE_CELL, 16);
    if let Some(arr) = s.follow_array(p, s.layout(12, 16), 4, cell, cells)? {
        for i in 0..cells {
            s.follow_string(arr.at(i * cell), 0)?;
        }
    }
    links.capture_string_table(s, p)?;
    s.pop()
}

pub(super) fn load_script_file(
    s: &mut ZoneStream<'_>,
    links: &mut dyn AssetLinkSink,
) -> Result<()> {
    s.walk_stage = "script_file";
    let p = s.alloc_load(4, s.layout(sz::SCRIPT_FILE, 40))?;
    let compressed_len = s.i32_at(p, s.layout(4, 8))?.max(0) as usize;
    let bytecode_len = s.i32_at(p, s.layout(12, 16))?.max(0) as usize;
    s.push(XFILE_BLOCK_VIRTUAL)?;
    follow_name(s, p, 0)?;
    s.push(XFILE_BLOCK_SCRIPT)?;
    let buf_ptr = always_array(s, p.at(s.layout(16, 24)), 1, compressed_len)?;
    s.pop()?;
    s.push(XFILE_BLOCK_SCRIPT)?;
    let code_ptr = always_array(s, p.at(s.layout(20, 32)), 1, bytecode_len)?;
    s.pop()?;
    if let Some(px) = buf_ptr {
        if compressed_len > 0 {
            let raw = s.slice_at(px, 0, compressed_len).unwrap_or(&[]);
            let name = linked_name(s, p);
            if !name.is_empty() {
                links.capture_raw_file(name, raw, true)?;
                if let Some(code) = code_ptr {
                    links.capture_script_file(name, raw, s.slice_at(code, 0, bytecode_len)?)?;
                }
            }
        }
    }
    s.pop()
}

pub(super) fn load_phys_preset(s: &mut ZoneStream<'_>) -> Result<()> {
    s.walk_stage = "phys_preset";
    let p = s.alloc_load(4, s.layout(sz::PHYS_PRESET, 80))?;
    s.push(XFILE_BLOCK_VIRTUAL)?;
    follow_name(s, p, 0)?;
    follow_name(s, p, s.layout(28, 32))?;
    s.pop()
}

pub(super) fn load_localize(s: &mut ZoneStream<'_>, links: &mut dyn AssetLinkSink) -> Result<()> {
    s.walk_stage = "localize";
    let p = s.alloc_load(4, s.layout(sz::LOCALIZE_ENTRY, 16))?;
    s.push(XFILE_BLOCK_VIRTUAL)?;
    let value = s.follow_string(p, 0)?;
    let name = s.follow_string(p, s.layout(4, 8))?;
    if let (Some(name), Some(value)) = (name, value) {
        let name = s.cstr(name)?;
        let value = s.cstr_bytes(value)?;
        links.capture_localize(name, value)?;
    }
    s.pop()
}

pub(super) fn load_font(s: &mut ZoneStream<'_>, links: &mut dyn AssetLinkSink) -> Result<()> {
    s.walk_stage = "font";
    let p = s.alloc_load(4, s.layout(sz::FONT, 40))?;
    let pixel_height = s.i32_at(p, s.layout(sz::FONT_PIXEL_HEIGHT_OFF, 8))?;
    let glyph_count = s.i32_at(p, s.layout(sz::FONT_GLYPH_COUNT_OFF, 12))?.max(0) as usize;
    s.push(XFILE_BLOCK_VIRTUAL)?;
    follow_name(s, p, 0)?;
    let material_name = load_font_material(s, links, p.at(s.layout(sz::FONT_MATERIAL_OFF, 16)))?;
    let glow_material_name =
        load_font_material(s, links, p.at(s.layout(sz::FONT_GLOW_MATERIAL_OFF, 24)))?;
    let glyphs = s.plain_array(
        p,
        s.layout(sz::FONT_GLYPHS_OFF, 32),
        4,
        sz::GLYPH,
        glyph_count,
    )?;
    let name = match s.ptr_at(p, 0)? {
        ZonePtr::Offset(q) => Some(s.resolve_alias(q)),
        _ => None,
    };
    links.capture_font(
        s,
        FontDefGeometry {
            name,
            pixel_height,
            glyphs,
            glyph_count,
            material_name,
            glow_material_name,
        },
    )?;
    s.pop()
}

fn load_font_material(
    s: &mut ZoneStream<'_>,
    links: &mut dyn AssetLinkSink,
    slot: crate::zone::Ptr,
) -> Result<Option<crate::zone::Ptr>> {
    let fresh = load_asset_at_observed(s, crate::asset_type::AssetType::Material, slot, links)?;
    Ok(if fresh {
        s.latest_material().and_then(|g| g.name)
    } else {
        material_name_at(s, links, slot)
    })
}

pub(super) fn load_leaderboard_def(s: &mut ZoneStream<'_>) -> Result<()> {
    s.walk_stage = "leaderboard";
    let p = s.alloc_load(4, s.layout(sz::LEADERBOARD_DEF, 40))?;
    let column_count = s.i32_at(p, s.layout(8, 12))?.max(0) as usize;

    s.push(XFILE_BLOCK_VIRTUAL)?;
    follow_name(s, p, 0)?;
    if s.begin_body(p.at(s.layout(20, 24)))? {
        let column_def = s.layout(sz::LB_COLUMN_DEF, 56);
        let stat_name = s.layout(16, 24);
        s.walk_stage = "leaderboard.columns";
        let columns = s.alloc_load(4, column_def * column_count)?;
        for index in 0..column_count {
            let column = columns.at(index * column_def);
            s.follow_string(column, 0)?;
            s.follow_string(column, stat_name)?;
        }
    }
    s.pop()
}

pub(super) fn load_structured_data_def_set(
    s: &mut ZoneStream<'_>,
    links: &mut dyn AssetLinkSink,
) -> Result<()> {
    s.walk_stage = "structured_data";
    let p = s.alloc_load(4, s.layout(sz::STRUCTURED_DATA_DEF_SET, 24))?;
    let def_count = s.i32_at(p, s.layout(4, 8))?.max(0) as usize;

    s.push(XFILE_BLOCK_VIRTUAL)?;
    follow_name(s, p, 0)?;
    let def = s.layout(sz::STRUCTURED_DATA_DEF, 88);
    if let Some(defs) = s.follow_array(p, s.layout(8, 16), 4, def, def_count)? {
        for index in 0..def_count {
            load_structured_data_def(s, defs.at(index * def))?;
        }
    }
    links.capture_structured_data_def_set(s, p)?;
    s.pop()
}

fn load_structured_data_def(s: &mut ZoneStream<'_>, p: crate::zone::Ptr) -> Result<()> {
    let enum_count = s.i32_at(p, 8)?.max(0) as usize;
    let enum_row = s.layout(sz::STRUCTURED_DATA_ENUM, 16);
    if let Some(enums) = s.follow_array(p, s.layout(12, 16), 4, enum_row, enum_count)? {
        let entry = s.layout(sz::STRUCTURED_DATA_ENUM_ENTRY, 16);
        for index in 0..enum_count {
            let row = enums.at(index * enum_row);
            let entry_count = s.i32_at(row, 0)?.max(0) as usize;
            if let Some(entries) = s.follow_array(row, 8, 4, entry, entry_count)? {
                for entry_index in 0..entry_count {
                    s.follow_string(entries.at(entry_index * entry), 0)?;
                }
            }
        }
    }

    let struct_count = s.i32_at(p, s.layout(16, 24))?.max(0) as usize;
    let struct_row = s.layout(sz::STRUCTURED_DATA_STRUCT, 24);
    if let Some(structs) = s.follow_array(p, s.layout(20, 32), 4, struct_row, struct_count)? {
        let property = s.layout(sz::STRUCTURED_DATA_STRUCT_PROPERTY, 24);
        for index in 0..struct_count {
            let row = structs.at(index * struct_row);
            let property_count = s.i32_at(row, 0)?.max(0) as usize;
            if let Some(properties) =
                s.follow_array(row, s.layout(4, 8), 4, property, property_count)?
            {
                for property_index in 0..property_count {
                    s.follow_string(properties.at(property_index * property), 0)?;
                }
            }
        }
    }

    let indexed_count = s.i32_at(p, s.layout(24, 40))?.max(0) as usize;
    s.plain_array(
        p,
        s.layout(28, 48),
        4,
        sz::STRUCTURED_DATA_ARRAY,
        indexed_count,
    )?;
    let enumed_count = s.i32_at(p, s.layout(32, 56))?.max(0) as usize;
    s.plain_array(
        p,
        s.layout(36, 64),
        4,
        sz::STRUCTURED_DATA_ARRAY,
        enumed_count,
    )?;
    Ok(())
}
