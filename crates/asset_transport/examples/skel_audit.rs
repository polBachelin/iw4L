// Temporary audit: dump IW5 XModel skeletons, XAnim bone lists and script/raw file strings.
use fastfile_iw5::{AssetType, Ptr, ScriptStrings, ZoneStream};

#[derive(Default)]
struct Sink {
    strings: ScriptStrings,
    zone: String,
}

impl fastfile_iw5::AssetLinkSink for Sink {
    fn loaded(
        &mut self,
        s: &ZoneStream<'_>,
        ty: AssetType,
        _slot: Ptr,
        _insert_slot: Option<Ptr>,
    ) -> fastfile_iw5::Result<()> {
        if ty != AssetType::XModel {
            return Ok(());
        }
        let Some(g) = s.latest_xmodel() else {
            return Ok(());
        };
        let name = g.name.and_then(|p| s.cstr(p).ok()).unwrap_or("?");
        let mut bones = Vec::new();
        if let Some(arr) = g.bone_names {
            for i in 0..g.num_bones {
                let id = s.u16_at(arr, i * 2).unwrap_or(0);
                let bone = self.strings.get(s, id).unwrap_or("");
                let parent = if i < g.num_root_bones {
                    String::new()
                } else {
                    g.parent_list
                        .and_then(|p| s.u8_at(p, i - g.num_root_bones).ok())
                        .map(|step| (i as i64 - step as i64).to_string())
                        .unwrap_or_default()
                };
                let parent = if i < g.num_root_bones {
                    parent
                } else {
                    let c = i - g.num_root_bones;
                    let q: Vec<i16> = (0..4)
                        .map(|k| {
                            g.quats
                                .and_then(|a| s.i16_at(a, c * 8 + k * 2).ok())
                                .unwrap_or(0)
                        })
                        .collect();
                    let t: Vec<f32> = (0..3)
                        .map(|k| {
                            g.trans
                                .and_then(|a| s.f32_at(a, c * 12 + k * 4).ok())
                                .unwrap_or(0.0)
                        })
                        .collect();
                    format!("{parent}|{:?}|{:.2?}", q, t)
                        .replace(", ", " ")
                        .replace(',', " ")
                };
                bones.push(format!("{bone}<{parent}"));
            }
        }
        println!("MODEL\t{}\t{name}\t{}", self.zone, bones.join(","));
        Ok(())
    }

    fn alias(&mut self, _ty: AssetType, _slot: Ptr, _target: Ptr) -> fastfile_iw5::Result<()> {
        Ok(())
    }

    fn capture_xanim(
        &mut self,
        s: &ZoneStream<'_>,
        g: fastfile_iw5::XAnimPartsGeometry,
    ) -> fastfile_iw5::Result<()> {
        let name = g.name.and_then(|p| s.cstr(p).ok()).unwrap_or("?");
        let mut names = Vec::new();
        if let Some(arr) = g.names {
            for i in 0..g.bone_count[9] as usize {
                let id = s.u16_at(arr, i * 2).unwrap_or(0);
                names.push(self.strings.get(s, id).unwrap_or("").to_owned());
            }
        }
        println!("ANIM\t{}\t{name}\t{}", self.zone, names.join(","));
        Ok(())
    }

    fn capture_script_file(
        &mut self,
        name: &str,
        stack: &[u8],
        bytecode: &[u8],
    ) -> fastfile_iw5::Result<()> {
        println!(
            "SCRIPT\t{}\t{name}\t{}",
            self.zone,
            printable(&[stack, bytecode].concat())
        );
        Ok(())
    }

    fn capture_raw_file(
        &mut self,
        name: &str,
        data: &[u8],
        zlib: bool,
    ) -> fastfile_iw5::Result<()> {
        let data = if zlib {
            miniz_oxide::inflate::decompress_to_vec_zlib(data).unwrap_or_else(|_| data.to_vec())
        } else {
            data.to_vec()
        };
        println!("RAW\t{}\t{name}\t{}", self.zone, printable(&data));
        Ok(())
    }
}

fn printable(data: &[u8]) -> String {
    let data = miniz_oxide::inflate::decompress_to_vec_zlib(data).unwrap_or_else(|_| data.to_vec());
    data.split(|b| !(0x20..0x7f).contains(b))
        .filter(|w| w.len() >= 4)
        .map(|w| String::from_utf8_lossy(w).replace('\t', " "))
        .collect::<Vec<_>>()
        .join("|")
}

impl fastfile_iw5::AssetSink for Sink {
    fn set_script_strings(&mut self, strings: ScriptStrings) {
        self.strings = strings;
    }

    fn load_asset(
        &mut self,
        s: &mut ZoneStream<'_>,
        _index: usize,
        ty: AssetType,
        slot: Ptr,
    ) -> fastfile_iw5::Result<()> {
        println!("ASSET\t{}\t{:?}", self.zone, ty);
        fastfile_iw5::load_asset_at_observed(s, ty, slot, self).map(|_| ())
    }
}

fn main() {
    for path in std::env::args().skip(1) {
        let image = asset_transport::open_zone(&path).expect("open");
        let header = image.iw5_header().expect("iw5 header");
        let mut memory = asset_transport::Iw5ZoneMemory::for_header(&header);
        let mut stream = memory.stream(&image.bytes).expect("stream");
        let zone = std::path::Path::new(&path)
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let mut sink = Sink {
            zone: zone.clone(),
            ..Default::default()
        };
        if let Err(e) = fastfile_iw5::load_zone(&mut stream, &mut sink) {
            eprintln!("{zone}: walk stopped: {e}");
        }
    }
}
