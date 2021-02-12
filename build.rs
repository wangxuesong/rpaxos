fn main() {
    tonic_build::configure()
        .out_dir("src")
        .compile(&["proto/paxos.proto"], &["proto"])
        .expect("Failed to compile proto")
}
