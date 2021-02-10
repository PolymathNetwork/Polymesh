use wasm_builder_runner::WasmBuilder;

const BUILDER_REPO: &str = "https://github.com/PolymathNetwork/substrate-wasm-builder.git";
const BUILDER_REV: &str = "b0303c15b662dd768bfe808e3ed47ec65305fe2a";

pub fn build() {
    WasmBuilder::new()
        .with_current_project()
        .with_wasm_builder_from_git(BUILDER_REPO, BUILDER_REV)
        .export_heap_base()
        .import_memory()
        .build()
}
