use clap::Parser;
use log::debug;
use mzdata::{
    io::{proxi::PROXISpectrum, usi, MZReader},
    prelude::*, spectrum::MultiLayerSpectrum,
};
use std::{fs, io, path::PathBuf, process::exit};

#[derive(Debug, clap::Parser)]
struct App {
    usi: String,
    #[arg(default_value = ".")]
    root: PathBuf,
}

impl App {
    fn dataset_root(&self, ident: &usi::USI) -> Option<PathBuf> {
        let dset_root = self.root.join(&ident.dataset);
        if !dset_root.exists() {
            None
        } else {
            Some(dset_root)
        }
    }

    fn find_file(&self, dset_root: &PathBuf, ident: &usi::USI) -> io::Result<PathBuf> {
        for leaf in fs::read_dir(dset_root)? {
            match leaf {
                Ok(leaf) => {
                    let os_file_name = leaf.file_name();
                    let leaf_file_name = os_file_name.to_str().unwrap_or_else(|| {
                        debug!("Failed to parse file name {}", leaf.path().display());
                        ""
                    });
                    debug!("Visiting {leaf_file_name}");
                    if leaf_file_name.starts_with(&ident.run_name) {
                        debug!("Found run!");
                        return Ok(leaf.path());
                    }
                }
                Err(e) => {
                    debug!("Got an error while listing files: {e}")
                }
            }
        }

        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "Could not find a file for {ident} under {}",
                dset_root.display()
            ),
        ));
    }

    fn get_spectrum_from_file(&self, data_path: &PathBuf, ident: &usi::USI) -> Option<MultiLayerSpectrum> {
        let mut reader = MZReader::open_path(data_path).ok()?;
        if let Some(idx) = ident.identifier.as_ref() {
            let mut spec = match idx {
                usi::Identifier::Scan(scan) => {
                    reader.get_spectrum_by_index((*scan).saturating_sub(1) as usize)
                }
                usi::Identifier::Index(index) => {
                    reader.get_spectrum_by_index((*index) as usize)
                }
                usi::Identifier::NativeID(_items) => todo!(),
            };
            if let Some(spec) = spec.as_mut() {
                if let Err(e) = spec.try_build_peaks() {
                    debug!("Failed to load peaks directly: {e}")
                }
                if spec.peaks.is_none() {
                    spec.pick_peaks(1.0).unwrap();
                }
                debug!("Found {} peaks", spec.peaks().len());
            }
            spec
        } else {
            None
        }
    }
}

fn main() -> io::Result<()> {
    env_logger::init();
    let args = App::parse();

    let ident: usi::USI = args.usi.parse().unwrap();
    debug!("got {ident}");

    let dset_root = args.dataset_root(&ident).unwrap_or_else(|| {
        debug!("Path for dataset {} does not exist!", ident.dataset);
        exit(1)
    });
    debug!("Searching from {}", dset_root.display());

    let mz_file_path = args.find_file(&dset_root, &ident)?;

    let spec = args.get_spectrum_from_file(
        &mz_file_path,
        &ident
    ).unwrap();

    let mut proxi_spec = PROXISpectrum::from(&spec);
    proxi_spec.usi = Some(ident.clone());
    let repr = serde_json::to_string_pretty(&proxi_spec)?;
    println!("{repr}");

    Ok(())
}
