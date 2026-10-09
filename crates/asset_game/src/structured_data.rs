use structured_data_iw4::{
    DataType, Definition, DefinitionSet, Enum, EnumArray, EnumEntry, IndexedArray, Property, Struct,
};

/// The two zone formats share the definition layout except for the width of
/// a struct property row.
trait Source {
    type Ptr: Copy;
    fn layout(&self, x86: usize, x64: usize) -> usize;
    fn property_stride(&self) -> usize;
    fn at(p: Self::Ptr, offset: usize) -> Self::Ptr;
    fn pointer(&self, p: Self::Ptr, offset: usize) -> Result<Self::Ptr, String>;
    fn word(&self, p: Self::Ptr, offset: usize) -> Result<usize, String>;
    fn half(&self, p: Self::Ptr, offset: usize) -> Result<u16, String>;
    fn text(&self, p: Self::Ptr) -> Result<String, String>;
    fn check(&self, p: Self::Ptr, length: usize) -> Result<(), String>;
}

macro_rules! source {
    ($zone:ident, $property:expr) => {
        impl Source for $zone::ZoneStream<'_> {
            type Ptr = $zone::Ptr;
            fn layout(&self, x86: usize, x64: usize) -> usize {
                $zone::ZoneStream::layout(self, x86, x64)
            }
            fn property_stride(&self) -> usize {
                $zone::ZoneStream::layout(self, $property, 24)
            }
            fn at(p: Self::Ptr, offset: usize) -> Self::Ptr {
                p.at(offset)
            }
            fn pointer(&self, p: Self::Ptr, offset: usize) -> Result<Self::Ptr, String> {
                match self.ptr_at(p, offset).map_err(|e| e.to_string())? {
                    $zone::ZonePtr::Offset(p) => Ok(self.resolve_alias(p)),
                    _ => Err("unresolved structured-data pointer".into()),
                }
            }
            fn word(&self, p: Self::Ptr, offset: usize) -> Result<usize, String> {
                Ok(self.u32_at(p, offset).map_err(|e| e.to_string())? as usize)
            }
            fn half(&self, p: Self::Ptr, offset: usize) -> Result<u16, String> {
                self.u16_at(p, offset).map_err(|e| e.to_string())
            }
            fn text(&self, p: Self::Ptr) -> Result<String, String> {
                Ok(self.cstr(p).map_err(|e| e.to_string())?.to_owned())
            }
            fn check(&self, p: Self::Ptr, length: usize) -> Result<(), String> {
                self.slice_at(p, 0, length)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
        }
    };
}

source!(fastfile_iw4, 16);
source!(
    fastfile_iw5,
    fastfile_iw5::size::STRUCTURED_DATA_STRUCT_PROPERTY
);

fn string<S: Source>(s: &S, p: S::Ptr, offset: usize) -> Result<String, String> {
    s.text(s.pointer(p, offset)?)
}
fn array<S: Source>(
    s: &S,
    p: S::Ptr,
    offset: usize,
    count: usize,
    stride: usize,
) -> Result<Option<S::Ptr>, String> {
    if count == 0 {
        return Ok(None);
    }
    let p = s.pointer(p, offset)?;
    let length = count
        .checked_mul(stride)
        .ok_or("structured-data array size overflow")?;
    s.check(p, length)?;
    Ok(Some(p))
}
fn ty<S: Source>(s: &S, p: S::Ptr, offset: usize) -> Result<DataType, String> {
    let category = s.word(p, offset)?;
    let value = s.word(p, offset + 4)?;
    Ok(match category {
        0 => DataType::Int,
        1 => DataType::Byte,
        2 => DataType::Bool,
        3 => DataType::String(value),
        4 => DataType::Enum(value),
        5 => DataType::Struct(value),
        6 => DataType::IndexedArray(value),
        7 => DataType::EnumArray(value),
        8 => DataType::Float,
        9 => DataType::Short,
        _ => return Err(format!("invalid structured-data category {category}")),
    })
}

pub fn capture_iw4_structured_data_def_set(
    s: &fastfile_iw4::ZoneStream<'_>,
    header: fastfile_iw4::Ptr,
) -> Result<(String, DefinitionSet), String> {
    capture_definition_set(s, header)
}

pub fn capture_iw5_structured_data_def_set(
    s: &fastfile_iw5::ZoneStream<'_>,
    header: fastfile_iw5::Ptr,
) -> Result<(String, DefinitionSet), String> {
    capture_definition_set(s, header)
}

fn capture_definition_set<S: Source>(
    s: &S,
    header: S::Ptr,
) -> Result<(String, DefinitionSet), String> {
    let name = string(s, header, 0)?;
    let count = s.word(header, s.layout(4, 8))?;
    let mut definitions = Vec::new();
    if let Some(rows) = array(s, header, s.layout(8, 16), count, s.layout(52, 88))? {
        for i in 0..count {
            definitions.push(
                capture_definition(s, S::at(rows, i * s.layout(52, 88)))
                    .map_err(|error| format!("{name}: {error}"))?,
            );
        }
    }
    Ok((name, DefinitionSet { definitions }))
}
fn capture_definition<S: Source>(s: &S, p: S::Ptr) -> Result<Definition, String> {
    let mut definition = Definition {
        version: s.word(p, 0)? as i32,
        checksum: s.word(p, 4)? as u32,
        root: ty(s, p, s.layout(40, 72))?,
        size: s.word(p, s.layout(48, 80))?,
        enums: Vec::new(),
        structs: Vec::new(),
        indexed_arrays: Vec::new(),
        enum_arrays: Vec::new(),
    };
    let count = s.word(p, 8)?;
    if let Some(rows) = array(s, p, s.layout(12, 16), count, s.layout(12, 16))? {
        for i in 0..count {
            let row = S::at(rows, i * s.layout(12, 16));
            let count = s.word(row, 0)?;
            let mut entries = Vec::new();
            if let Some(rows) = array(s, row, 8, count, s.layout(8, 16))? {
                for i in 0..count {
                    let row = S::at(rows, i * s.layout(8, 16));
                    entries.push(EnumEntry {
                        name: string(s, row, 0)?,
                        index: s.half(row, s.layout(4, 8))?,
                    });
                }
            }
            definition.enums.push(Enum {
                reserved: s.word(row, 4)?,
                entries,
            });
        }
    }
    let count = s.word(p, s.layout(16, 24))?;
    if let Some(rows) = array(s, p, s.layout(20, 32), count, s.layout(16, 24))? {
        for i in 0..count {
            let row = S::at(rows, i * s.layout(16, 24));
            let count = s.word(row, 0)?;
            let mut properties = Vec::new();
            if let Some(rows) = array(s, row, s.layout(4, 8), count, s.property_stride())? {
                for i in 0..count {
                    let row = S::at(rows, i * s.property_stride());
                    properties.push(Property {
                        name: string(s, row, 0)?,
                        ty: ty(s, row, s.layout(4, 8))?,
                        offset: s.word(row, s.layout(12, 16))?,
                    });
                }
            }
            definition.structs.push(Struct {
                properties,
                size: s.word(row, s.layout(8, 16))?,
                bit_offset: s.word(row, s.layout(12, 20))?,
            });
        }
    }
    let count = s.word(p, s.layout(24, 40))?;
    if let Some(rows) = array(s, p, s.layout(28, 48), count, 16)? {
        for i in 0..count {
            let row = S::at(rows, i * 16);
            definition.indexed_arrays.push(IndexedArray {
                count: s.word(row, 0)?,
                element: ty(s, row, 4)?,
                stride: s.word(row, 12)?,
            });
        }
    }
    let count = s.word(p, s.layout(32, 56))?;
    if let Some(rows) = array(s, p, s.layout(36, 64), count, 16)? {
        for i in 0..count {
            let row = S::at(rows, i * 16);
            definition.enum_arrays.push(EnumArray {
                enumeration: s.word(row, 0)?,
                element: ty(s, row, 4)?,
                stride: s.word(row, 12)?,
            });
        }
    }
    definition
        .validate()
        .map_err(|e| format!("invalid structured-data definition: {e:?}"))?;
    Ok(definition)
}
