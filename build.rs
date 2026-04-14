fn main() {
    let mut res = winres::WindowsResource::new();
    res.set_icon("caps_switch.ico");
    res.compile().unwrap();
}
