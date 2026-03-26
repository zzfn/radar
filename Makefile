BINARY    := radar
TARGET    := target/release/$(BINARY)
SCRIPTS   := skills/radar/scripts

.PHONY: build test release clean

## 开发构建（debug）
build:
	cargo build

## 运行测试
test:
	cargo test

## 发布构建，并将产物复制到 scripts/
release:
	cargo build --release
	@mkdir -p $(SCRIPTS)
	@cp $(TARGET) $(SCRIPTS)/$(BINARY)
	@chmod +x $(SCRIPTS)/$(BINARY)
	@echo "✓  scripts/$(BINARY) $$($(SCRIPTS)/$(BINARY) --version 2>/dev/null | head -1)"
	@ls -lh $(SCRIPTS)/$(BINARY)

## 清理构建产物
clean:
	cargo clean
	@rm -f $(SCRIPTS)/$(BINARY)
