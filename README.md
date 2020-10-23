# `ghost2zola`: migrate a ghost blog to zola

## Preparation

- Export your entire ghost directory into a tar:
  - The following data formats are supported:
    - `ghost.tar`
    - `ghost.tar.gz`
    - `ghost.tar.bz2`
  - This program analyzes the input file type, so no magic filenames are necessary.
- Note: unlike ghost's built-in data exports, this preserves media such as images.

## Usage

```
USAGE:
    ghost2zola [OPTIONS] <archive-path> <extract-path>

FLAGS:
    -h, --help
            Prints help information

    -V, --version
            Prints version information


OPTIONS:
        --prefix <prefix>
            Relative prefix within the archive

            In cases where the archive contains only a single blog, this is not necessary. When the archive contains
            several blogs, this can be set to any distinct prefix winnowing the selection to a single selection.

            If you're not sure what prefixes might be available, consider using the `find_ghost_db` tool.

ARGS:
    <archive-path>
            Path to a possibly-compressed tar archiving a ghost blog

    <extract-path>
            Path to the base directory into which the ghost blog should be expanded.

            Normally, this is the `content/blog` directory of your zola installation.
```
