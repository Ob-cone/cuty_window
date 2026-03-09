fn main() {
    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("Cuty_window.ico"); // 적용할 아이콘 파일 경로
        res.compile().unwrap();
    }
}
