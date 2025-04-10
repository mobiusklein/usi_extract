use clap::Parser;
use log::debug;
use mzdata::{
    io::{proxi::PROXISpectrum, usi, MZReader},
    prelude::*,
    spectrum::MultiLayerSpectrum,
    Param,
};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Resolve a USI from the file system.
///
/// Datasets are queried under each prefix until the requested file and
/// spectrum is located. MGF files are explicitly ignored, but all other
/// supported MS data files will be queried in whatever order the file
/// system lists them.
#[derive(Debug, clap::Parser)]
struct App {
    #[arg(help = "The USI to search for")]
    usi: usi::USI,

    #[arg(short, long, help = "Read only spectrum metadata")]
    metadata_only: bool,

    /// Prefixes to visit when resolving datasets
    ///
    /// Suppose we had the USI `mzspec:PXD0012345:data_file:scan:232`, for each
    /// prefix we search `<prefix>/PXD0012345/data_file*`
    #[arg(short = 'p', long = "prefix", default_value = ".")]
    prefixes: Vec<PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct RepositoryPrefix {
    root: PathBuf,
}

impl RepositoryPrefix {
    fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn iter_ms_data_files<'a>(
        &'a self,
        ident: &'a usi::USI,
    ) -> io::Result<impl Iterator<Item = PathBuf> + 'a> {
        match fs::read_dir(&self.root.join(&ident.dataset)) {
            Ok(iter) => {
                let mut dirs: Vec<_> = iter.collect();
                dirs.sort_by_key(|a| {
                    a.as_ref()
                        .map(|a| -(a.file_name().len() as i64))
                        .unwrap_or_default()
                });
                let it = dirs.into_iter().filter_map(|leaf| match leaf {
                    Ok(leaf) => {
                        let os_file_name = leaf.file_name();
                        let leaf_file_name = os_file_name.to_str().unwrap_or_else(|| {
                            debug!("Failed to parse file name {}", leaf.path().display());
                            ""
                        });
                        let path_of = leaf.path();
                        let is_mgf = path_of
                            .extension()
                            .map(|ext| ext.to_ascii_lowercase() == "mgf")
                            .unwrap_or_default();
                        debug!("Visiting file {leaf_file_name}");
                        if leaf_file_name.starts_with(&ident.run_name) && !is_mgf {
                            debug!("Found run!");
                            return Some(path_of);
                        } else {
                            None
                        }
                    }
                    Err(e) => {
                        log::error!("read_dir::next failed to decode: {e}");
                        None
                    }
                });
                Ok(it)
            }
            Err(e) => {
                log::error!(
                    "read_dir failed while enumerating prefix {}: {e}",
                    self.root.display()
                );
                Err(e)
            }
        }
    }

    fn get_spectrum_from_file(
        &self,
        data_path: &Path,
        ident: &usi::USI,
        load_peaks: bool,
    ) -> Option<MultiLayerSpectrum> {
        let mut reader = MZReader::open_path(data_path)
            .inspect_err(|e| {
                log::error!("Failed to open `{}`: {e}", data_path.display());
            })
            .ok()?;
        if !load_peaks {
            reader.set_detail_level(mzdata::io::DetailLevel::MetadataOnly);
        }
        if let Some(idx) = ident.identifier.as_ref() {
            let mut spec = match idx {
                usi::Identifier::Scan(scan) => {
                    reader.get_spectrum_by_index((*scan).saturating_sub(1) as usize)
                }
                usi::Identifier::Index(index) => reader.get_spectrum_by_index((*index) as usize),
                usi::Identifier::NativeID(_items) => todo!(),
            };
            if let Some(spec) = spec.as_mut() {
                if let Err(e) = spec.try_build_peaks() {
                    debug!("Failed to load peaks directly: {e}")
                }
                if spec.peaks.is_none() {
                    spec.pick_peaks(1.0).unwrap();
                }
                spec.add_param(
                    Param::builder()
                        .name("number of peaks")
                        .curie(mzdata::curie!(MS:1008040))
                        .value(spec.peaks().len())
                        .build(),
                );
                debug!("Found {} peaks", spec.peaks().len());
            }
            spec
        } else {
            None
        }
    }

    fn find_spectrum(&self, ident: &usi::USI, load_peaks: bool) -> Option<MultiLayerSpectrum> {
        self.iter_ms_data_files(&ident)
            .inspect_err(|e| {
                log::error!("Failed to invoke read_dir: {e}");
            })
            .ok()?
            .filter_map(|p| self.get_spectrum_from_file(&p, &ident, load_peaks))
            .next()
    }
}

impl App {
    fn main(&self) -> io::Result<()> {
        let ident: usi::USI = self.usi.clone();
        debug!("got {ident}");

        let spec = self
            .prefixes
            .iter()
            .cloned()
            .map(RepositoryPrefix::new)
            .filter_map(|p| {
                debug!("Visiting {p:?}");
                p.find_spectrum(&ident, !self.metadata_only)
            })
            .next();

        if let Some(spec) = spec {
            let mut proxi_spec = PROXISpectrum::from(&spec);
            proxi_spec.usi = Some(ident.clone());
            let repr = serde_json::to_string_pretty(&proxi_spec)?;
            println!("{repr}");
            Ok(())
        } else {
            log::error!("Failed to locate spectrum for `{ident}`");
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Failed to locate spectrum for `{ident}`"),
            ))
        }
    }
}

fn main() -> io::Result<()> {
    env_logger::init();
    let args = App::parse();
    args.main()
}
