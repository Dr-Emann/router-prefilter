# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.6.1](https://github.com/Dr-Emann/router-prefilter/compare/v1.6.0...v1.6.1)

### Refactor


- Cargo clippy fixes - ([26a2663](https://github.com/Dr-Emann/router-prefilter/commit/26a2663ae80a864833d568d55d2ab0a316ad19a3))

### Testing


- Expand coverage of extract_prefixes and visit_match_regex - ([c89d303](https://github.com/Dr-Emann/router-prefilter/commit/c89d3032ef491e5500a168313597c29fdf877635))

### Miscellaneous Tasks


- Update release-plz config to change changelog generation - ([1e87f7e](https://github.com/Dr-Emann/router-prefilter/commit/1e87f7edaed796735e590909801ea8b97f71ab38))
- Add testing and ci validation - ([0f8ffe9](https://github.com/Dr-Emann/router-prefilter/commit/0f8ffe9cb448316944df2c6b58ade2fbefb4ae82))


## [1.6.0](https://github.com/Dr-Emann/router-prefilter/compare/v1.5.0...v1.6.0) - 2026-03-22

### Added

- always_possible_keys() and prefixes()

## [1.5.0](https://github.com/Dr-Emann/router-prefilter/compare/v1.4.0...v1.5.0) - 2026-03-19

### Added

- add the ability to examine the prefixes used for prefiltering

### Other

- add docs about the need to balance nested_start/finish

## [1.4.0](https://github.com/Dr-Emann/router-prefilter/compare/v1.3.3...v1.4.0) - 2026-03-18

### Added

- use BString more readily

## [1.3.3](https://github.com/Dr-Emann/router-prefilter/compare/v1.3.2...v1.3.3) - 2026-03-03

### Other

- add unit tests for intersect_prefix_expansions_into function
- add docs about what intersecting with prefix expansion means/does

## [1.3.2](https://github.com/Dr-Emann/router-prefilter/compare/v1.3.1...v1.3.2) - 2026-02-16

### Other

- add more documentation on how to call the visitor functions

## [1.3.1](https://github.com/Dr-Emann/router-prefilter/compare/v1.3.0...v1.3.1) - 2026-02-13

### Other

- *(deps)* update rand to published 0.10 version in dev dependencies

## [1.3.0](https://github.com/Dr-Emann/router-prefilter/compare/v1.2.0...v1.3.0) - 2026-02-08

### Other

- replace BTreeMap-based implmementation with a radix trie

## [1.2.0](https://github.com/Dr-Emann/router-prefilter/compare/v1.1.1...v1.2.0) - 2026-02-06

### Added

- implement more iterator traits for RouterPrefilterIter

### Fixed

- handle duplicate key insertions

### Other

- fix compilation of readme example by using as the library doc comment

## [1.1.1](https://github.com/Dr-Emann/router-prefilter/compare/v1.1.0...v1.1.1) - 2026-02-06

### Other

- optimize insert

## [1.1.0](https://github.com/Dr-Emann/router-prefilter/compare/v1.0.2...v1.1.0) - 2026-02-03

### Added

- add RouterPrefilter::clear method

## [1.0.2](https://github.com/Dr-Emann/router-prefilter/compare/v1.0.1...v1.0.2) - 2026-02-02

### Other

- Update docs

## [1.0.1](https://github.com/Dr-Emann/router-prefilter/compare/v1.0.0...v1.0.1) - 2026-02-02

### Other

- updates for package repo
