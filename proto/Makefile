.DEFAULT_GOAL := update-proto


SPEC_DIR := ./opamp-spec

# Update protos from spec submodule
.PHONY: update-proto
update-proto:
	git submodule update --init
	REBUILD_PROTO=true cargo build
	cargo fmt

.PHONY: clean
clean:
	rm -r $(SPEC_DIR)
