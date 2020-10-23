# `ghost2zola`: migrate a ghost blog to zola

## Usage

- Export your entire ghost directory into a tar:
  - The following data formats are supported:
    - `ghost.tar`
    - `ghost.tar.gz`
    - `ghost.tar.bz2`
  - This program analyzes the input file type, so no magic filenames are necessary.
- Note: unlike ghost's built-in data exports, this preserves media such as images.
