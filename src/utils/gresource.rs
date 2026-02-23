pub fn resource_path(path: &str) -> String {
    const RESOURCES: &str = "/io/github/lost-melody/NeoDock/";
    RESOURCES.to_owned() + path
}
