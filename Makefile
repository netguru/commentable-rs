# List of all produced Lambda functions
LAMBDAS := auth list-comments add-comment delete-comment add-reaction

# All source code files
SOURCE_CODE := $(wildcard src/*.rs) $(wildcard src/*/*.rs)

# Set proper variables depending on the platform
ifeq ($(TARGET_NATIVE), 1)
	DEBUG_DIR := target/debug
	RELEASE_DIR := target/release
	CARGO = ~/.cargo/bin/cargo
else
	DEBUG_DIR := target/x86_64-unknown-linux-musl/debug
	RELEASE_DIR := target/x86_64-unknown-linux-musl/release
	TARGET_FLAGS := --target x86_64-unknown-linux-musl
	CARGO = CC_x86_64_unknown_linux_musl=x86_64-linux-musl-gcc cargo
endif

# The location of binaries depends on the target
DEBUG_BINARIES := $(addprefix $(DEBUG_DIR)/,$(LAMBDAS))
RELEASE_BINARIES := $(addprefix $(RELEASE_DIR)/,$(LAMBDAS))

# Set the location of bootstraps (for SAM packaging)
DEBUG_BOOTSTRAPS_DIR := lambda/debug/bootstraps
RELEASE_BOOTSTRAPS_DIR := lambda/release/bootstraps
DEBUG_BOOTSTRAPS := $(addsuffix /bootstrap,$(addprefix $(DEBUG_BOOTSTRAPS_DIR)/,$(LAMBDAS)))
RELEASE_BOOTSTRAPS := $(addsuffix /bootstrap,$(addprefix $(RELEASE_BOOTSTRAPS_DIR)/,$(LAMBDAS)))

# aws sam cli ENV overrides
SAM_ENV := SAM_CLI_TELEMETRY=0

.PHONY: debug
debug: $(DEBUG_BOOTSTRAPS)

.PHONY: release
release: $(RELEASE_BOOTSTRAPS)

# The bootstraps depend on binaries
$(DEBUG_BOOTSTRAPS): $(DEBUG_BINARIES)
$(RELEASE_BOOTSTRAPS): $(RELEASE_BINARIES)

# Bootstraps reside in directories of the same name as the binary
$(DEBUG_BOOTSTRAPS_DIR)/%/bootstrap: $(DEBUG_DIR)/%
	mkdir -p $(DEBUG_BOOTSTRAPS_DIR)/$* && cp $< $@
$(RELEASE_BOOTSTRAPS_DIR)/%/bootstrap: $(RELEASE_DIR)/%
	mkdir -p $(RELEASE_BOOTSTRAPS_DIR)/$* && cp $< $@

# Binaries are created by cargo from files in the src/bin folder
$(DEBUG_BINARIES): $(SOURCE_CODE)
	$(CARGO) build $(TARGET_FLAGS)
$(RELEASE_BINARIES): $(SOURCE_CODE)
	$(CARGO) build --release $(TARGET_FLAGS)

# Remove all binaries and bootstraps (but don't remove dependencies)
.PHONY: clean
clean:
	rm -rf $(DEBUG_BOOTSTRAPS_DIR) $(RELEASE_BOOTSTRAPS_DIR)
	rm -f $(addprefix $(DEBUG_DIR)/,$(LAMBDAS))
	rm -f $(addprefix $(RELEASE_DIR)/,$(LAMBDAS))
	rm -f .cargo/.bucket-exists
	rm -f .cargo/.docker*

# Docker scripts
.PHONY: docker-debug
docker-debug: | .cargo/.docker-debug-image-exists
	docker run --rm -itv $(PWD):/app commentable-rs-debug

.cargo/.docker-debug-image-exists:
	docker build --rm lambda/debug -t commentable-rs-debug
	touch .cargo/.docker-debug-image-exists

.PHONY: docker-release
docker-release: | .cargo/.docker-release-image-exists
	docker run --rm -itv $(PWD):/app commentable-rs-release

.cargo/.docker-release-image-exists:
	docker build --rm --no-cache lambda/release -t commentable-rs-release
	touch .cargo/.docker-release-image-exists

# SAM scripts
.PHONY: run-debug
run-debug: docker-debug
	sam local start-api --template lambda/debug/template.yml

.PHONY: run-release
run-release: docker-release
	$(SAM_ENV) sam local start-api --template lambda/release/template.yml

.PHONY: deploy
deploy: package.yml
	$(SAM_ENV) sam deploy --template-file package.yml --stack-name commentable-rs --capabilities CAPABILITY_IAM
	rm package.yml

package.yml: docker-release | .cargo/.bucket-exists
	$(SAM_ENV) sam package --template-file lambda/release/template.yml --s3-bucket commentable-rs --output-template-file package.yml

.cargo/.bucket-exists:
	aws s3 mb s3://commentable-rs
	touch .cargo/.bucket-exists

# All-in-one scripts
.PHONY: install
install: deploy
