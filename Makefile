.PHONY: third-party-notices
third-party-notices:
	@echo "Checking third-party licenses..."
	@(cargo install --list | grep cargo-deny) || cargo install cargo-deny --locked
	@(cargo install --list | grep rust-licenses-noticer) || cargo install --git https://github.com/newrelic/rust-licenses-noticer.git --locked
	@LICENSES=$$(cargo deny -L off --all-features --locked --manifest-path ./Cargo.toml list -l crate -f json 2>&1); \
    $$HOME/.cargo/bin/rust-licenses-noticer --dependencies "$$(printf "%s " $$LICENSES)" --template-file "./THIRD_PARTY_NOTICES.md.tmpl" --output-file "./THIRD_PARTY_NOTICES.md"

.PHONY: third-party-notices-check
third-party-notices-check: third-party-notices
	@git diff --name-only | grep -q "THIRD_PARTY_NOTICES.md" && { echo "Third party notices out of date, please commit the changes to the THIRD_PARTY_NOTICES.md file.";  exit 1; } || exit 0
