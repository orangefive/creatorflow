fn main() {
    // Rust 测试二进制默认不带应用清单，Windows 会加载 comctl32 v5，
    // 缺少 TaskDialogIndirect 等入口点导致测试进程启动失败（0xc0000139）。
    // 这里只为测试目标（lib/bin 测试）声明 Common-Controls 6.0 依赖。
    #[cfg(windows)]
    {
        let manifest_dependency = "/MANIFESTDEPENDENCY:type='win32' \
            name='Microsoft.Windows.Common-Controls' version='6.0.0.0' \
            processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'";
        println!("cargo:rustc-link-arg-tests={manifest_dependency}");
    }

    tauri_build::build()
}
