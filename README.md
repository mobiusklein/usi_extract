# usi_extract

Find spectra from a set of file system prefixes via a [universal spectrum identifier (USI)](https://www.psidev.info/usi).

## Supported mass spectrometry data formats
- mzML
- Thermo RAW (requires .NET runtime)
- Bruker TDF

We explicitly do not support MGF files because they do not contain all spectra by index.

## How is the spectrum formatted?
The spectrum is formatted as described for the [PROXI schema](https://github.com/HUPO-PSI/proxi-schemas/blob/master/specs/swagger.yaml#L526-L560).

If the spectrum is not already centroided, it will be. If the spectrum is an ion mobility frame, ion mobility traces will be extracted and merged into single peaks.