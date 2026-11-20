//! Sous Windows : l'icône et les informations du programme dans l'exécutable.

fn main() {
    #[cfg(windows)]
    {
        let mut ressource = winresource::WindowsResource::new();
        ressource.set_icon("../ressources/icones/icon.ico");
        ressource.set("ProductName", "song-audio");
        ressource.set("FileDescription", "song-audio");
        if let Err(e) = ressource.compile() {
            // sans outil de ressources, l'exécutable fonctionne quand même (sans icône)
            println!("cargo:warning=icône de l'exécutable non intégrée : {e}");
        }
    }
    println!("cargo:rerun-if-changed=../ressources/icones/icon.ico");
}
