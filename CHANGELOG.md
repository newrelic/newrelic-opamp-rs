# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Unreleased section should follow [Release Toolkit](https://github.com/newrelic/release-toolkit/blob/main/README.md).

Remember that the keywords that you can use under Unreleased section are:
 - breaking    => Major
 - security    => Minor
 - enhancement => Minor
 - bugfix      => Patch

## Unreleased

## v0.2.0 - 2026-09-25

### 🚀 Enhancements
- OpAMP client now accepts disabling the compression via StartSettings

### ⛓️ Dependencies
- Updated rust crate rand to v0.10.3

## v0.1.0 - 2026-09-16

### 🚀 Enhancements
- OpAMP client logs a trace message even when it disconnects from the OpAMP server

### ⛓️ Dependencies
- Updated rust to v1.98.1
- Updated rust crate uuid to v1.26.1
- Updated rust crate crossbeam to v0.8.5
- Updated rust crate libflate to v2.3.2
- Updated rust crate rstest to 0.27.0
- Updated rust crate reqwest to v0.13.5

## v0.0.42 - 2026-08-25

### ⛓️ Dependencies
- Updated rust crate rand to 0.10.2
- Updated rust to v1.97.1
- Updated rust crate uuid to 1.25.0
- Updated rust crate thiserror to 2.0.20
- Updated rust crate libflate to 2.3.1
- Updated rust crate http to 1.5.0

## v0.0.41 - 2026-07-01

### 🐞 Bug fixes
- Decorates client decoder error message

### ⛓️ Dependencies
- Updated rust crate uuid to 1.23.4
- Updated rust crate mockall to 0.15.0
- Updated rust to v1.96.1

## v0.0.40 - 2026-06-16

### ⛓️ Dependencies
- Updated rust crate reqwest to 0.13.4
- Updated rust crate http to 1.4.2
- Updated rust to v1.96.0
- Updated rust crate uuid to 1.23.3
- Updated tokio-prost monorepo to 0.14.4

## v0.0.39 - 2026-05-18

### 🐞 Bug fixes
- Fixed custom capabilities

## v0.0.38 - 2026-05-15

### ⛓️ Dependencies
- Updated rust crate reqwest to 0.13.3
