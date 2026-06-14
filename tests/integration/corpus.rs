//! Testes de integração com corpus de vídeos reais do PRD Apêndice A.1.1
//!
//! Estes testes fazem requests HTTP reais para os provedores do pipeline.
//! Requerem conectividade com a internet. Marcados #[ignore] por padrão para não
//! quebrar CI offline. Para rodar:
//!
//! ```bash
//! cargo test --test corpus -- --ignored --nocapture
//! ```

use std::process::Command;

fn run_youtube_legend_cli(url: &str) -> std::process::Output {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    Command::new(cargo)
        .args(["run", "--quiet", "--", url])
        .arg("--timeout")
        .arg("60")
        .output()
        .expect("falha ao executar binário")
}

#[test]
#[ignore = "requer internet; rode com --ignored"]
fn extracts_nvz4vzz5hooy_legenda() {
    let output = run_youtube_legend_cli("https://youtu.be/NvZ4VZ5hooY");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "exit code: {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(!stdout.is_empty(), "stdout vazio para vídeo com legenda");
}

#[test]
#[ignore = "requer internet; rode com --ignored"]
fn extracts_ulnsasds8n0_legenda() {
    let output = run_youtube_legend_cli("https://youtu.be/ulNsa0sD8N0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "exit code: {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(!stdout.is_empty(), "stdout vazio para vídeo com legenda");
}

#[test]
#[ignore = "requer internet; rode com --ignored"]
fn extracts_86fawczie_4_legenda() {
    let output = run_youtube_legend_cli("https://youtu.be/86FAWCzIe_4");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "exit code: {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(!stdout.is_empty(), "stdout vazio para vídeo com legenda");
}

#[test]
#[ignore = "requer internet; rode com --ignored"]
fn tn08k_pwopk_returns_no_subtitle() {
    let output = run_youtube_legend_cli("https://youtu.be/Tn08k_PWOQk");
    assert_eq!(
        output.status.code(),
        Some(66),
        "esperado exit code 66 (EX_NOINPUT) para vídeo sem legenda, recebeu {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}
