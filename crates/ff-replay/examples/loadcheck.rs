//! Loads every bundle in `evidence/` and prints what each one supports.
//!
//! A quick check that both capture kinds parse, without standing up the API.

fn main() {
    for dir in ["evidence/eks-recovery", "evidence/eks-live"] {
        match ff_replay::ReplayBundle::load(dir) {
            Ok(b) => {
                println!(
                    "OK   {dir}\n     kind={:?} events={} chapters={} claims={} chain={}+{}",
                    b.kind,
                    b.timeline.len(),
                    b.chapters.len(),
                    b.claims.len(),
                    b.chain.links.len(),
                    b.chain.edges.len()
                );
                for c in &b.claims {
                    println!("       [{:<22}] {}", format!("{:?}", c.basis), c.id);
                }
            }
            Err(e) => println!("FAIL {dir}\n     {e}"),
        }
    }
}
