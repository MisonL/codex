use std::fs;
use std::path::Path;
use std::path::PathBuf;

use codex_core::features::Feature;
use codex_protocol::config_types::WebSearchMode;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::test_codex;
use image::ImageBuffer;
use image::Rgba;
use pretty_assertions::assert_eq;

pub(crate) struct ToolchainFixture {
    pub(crate) src_dir: PathBuf,
    pub(crate) asset_path: PathBuf,
}

impl ToolchainFixture {
    pub(crate) fn write(cwd: &Path) -> anyhow::Result<Self> {
        let src_dir = cwd.join("src");
        let asset_dir = cwd.join("assets");
        fs::create_dir_all(&src_dir)?;
        fs::create_dir_all(&asset_dir)?;
        fs::write(
            src_dir.join("ledger.txt"),
            "total=3\nbonus=4\nstatus=pending\n",
        )?;
        let asset_path = asset_dir.join("evidence.png");
        ImageBuffer::from_pixel(32, 16, Rgba([0u8, 90, 180, 255])).save(&asset_path)?;
        Ok(Self {
            src_dir,
            asset_path,
        })
    }

    pub(crate) fn assert_files(&self) -> anyhow::Result<()> {
        assert_eq!(
            fs::read_to_string(self.src_dir.join("generated.txt"))?,
            "sum=7\nsource=exec-write-stdin\n"
        );
        assert_eq!(
            fs::read_to_string(self.src_dir.join("result.txt"))?,
            "status=done\nsum=7\ndoubled=14\n"
        );
        Ok(())
    }
}

pub(crate) fn test_builder() -> TestCodexBuilder {
    test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_config(|config| {
            config.include_apply_patch_tool = true;
            config.use_experimental_unified_exec_tool = true;
            let web_search_mode = config.web_search_mode.set(WebSearchMode::Live);
            assert!(
                web_search_mode.is_ok(),
                "test config should allow web search mode: {web_search_mode:?}"
            );
            for feature in [
                Feature::ApplyPatchFreeform,
                Feature::DefaultModeRequestUserInput,
                Feature::ImageGeneration,
                Feature::JsRepl,
                Feature::RequestPermissions,
                Feature::RequestPermissionsTool,
                Feature::UnifiedExec,
            ] {
                let enabled = config.features.enable(feature);
                assert!(
                    enabled.is_ok(),
                    "test config should allow feature update for {feature:?}: {enabled:?}"
                );
            }
        })
}
