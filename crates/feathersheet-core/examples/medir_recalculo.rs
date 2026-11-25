//! Recálculo em planilha grande (#142): `cargo run --release --example medir_recalculo [linhas]`.
//! A números; B `=A*2` e C `=B+A` (fórmula por padrão); D soma acumulada (`=D anterior + C`, a corrente
//! mais longa); E1 `=SOMA(C:C)`, E2 `=SOMASE(A:A;">50";C:C)`, E3 `=PROCV(…;A:B;2;0)`.
use feathersheet_core::planilha::workbook::Workbook;
use std::time::Instant;

fn ms(t: Instant) -> String { format!("{:.0} ms", t.elapsed().as_secs_f64() * 1000.0) }

fn main() {
    let n: u32 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(1_000_000);
    let mut w = Workbook::new();
    w.definir_calculo_manual(true);
    let t = Instant::now();
    for r in 0..n {
        let l = r + 1;
        w.escrever_celula(0, r, 0, ((r * 37) % 100).to_string());
        w.escrever_celula(0, r, 1, format!("=A{l}*2"));
        w.escrever_celula(0, r, 2, format!("=B{l}+A{l}"));
        w.escrever_celula(0, r, 3, if r == 0 { "=C1".into() } else { format!("=D{r}+C{l}") });
    }
    w.escrever_celula(0, 0, 4, "=SOMA(C:C)".into());
    w.escrever_celula(0, 1, 4, "=SOMASE(A:A;\">50\";C:C)".into());
    w.escrever_celula(0, 2, 4, format!("=PROCV({};A:B;2;0)", 99));
    println!("{n} linhas · escrever as fórmulas (cálculo manual): {}", ms(t));
    w.definir_calculo_manual(false);
    let t = Instant::now();
    w.calcular_tudo();
    println!("recalcular tudo (Ctrl+Alt+F9): {} · E1 = {:?}", ms(t), w.valor(0, 0, 4));
    let t = Instant::now();
    w.escrever_celula(0, n / 2, 0, "7".into());
    w.calcular_agora();
    println!("editar A{} (meio; D corre até o fim): {}", n / 2 + 1, ms(t));
    let t = Instant::now();
    w.escrever_celula(0, 0, 0, "8".into());
    w.calcular_agora();
    println!("editar A1 (a corrente D inteira): {} · D{n} = {:?}", ms(t), w.valor(0, n - 1, 3));
    let t = Instant::now();
    w.escrever_celula(0, 5, 6, "1".into());
    w.calcular_agora();
    println!("editar G6 (sem dependentes): {}", ms(t));
}
