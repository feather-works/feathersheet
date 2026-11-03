//! Gera um `.xlsx` de exemplo com um controle de formulário de cada tipo, cada um ligado a uma
//! célula e com uma fórmula ao lado que reage a ele (para aprender a usar e para testar).
//!
//! Uso (a partir de `feathersheet/crates/feathersheet-io`):
//!   cargo run --release --example exemplo_controles -- <arquivo.xlsx>

use feathersheet_core::planilha::workbook::Workbook;
use feathersheet_io::objetos_aba::{Ancora, ControleFormulario, ObjetoSolto, ObjetosAba, TipoControleForm as T};

/// Controle da linha `l0` até a `l1` (base 0) na coluna B, com `largura` px.
fn controle(c: ControleFormulario, l0: u32, l1: u32, largura: f32, texto: Option<&str>) -> ObjetoSolto {
    let mut o = ObjetoSolto::novo_controle(c, Ancora { linha: l0, coluna: 1, dx: 4.0, dy: 2.0 }, Ancora { linha: l1, coluna: 1, dx: 4.0 + largura, dy: 0.0 });
    o.texto = texto.map(str::to_owned);
    o
}

fn main() {
    let saida = std::env::args().nth(1).expect("uso: exemplo_controles <arquivo.xlsx>");
    let mut wb = Workbook::new();
    wb.renomear_aba(0, "Controles");
    let linhas: &[(u32, u32, &str)] = &[
        (0, 0, "Exemplo de controles de formulário (clique nos controles da coluna B)"),
        (1, 0, "O que é"), (1, 2, "Célula ligada"), (1, 3, "Fórmula que usa a célula"),
        (2, 0, "Botão de rotação: soma/diminui de 1 em 1 (1 a 10)"), (2, 2, "3"), (2, 3, "=C3*10"),
        (5, 0, "Caixa de seleção: VERDADEIRO / FALSO"), (5, 2, "FALSO"), (5, 3, "=SE(C6;\"Com frete: R$ 15\";\"Sem frete\")"),
        (7, 0, "Caixa de listagem: número do item escolhido"), (7, 2, "2"), (7, 3, "=ÍNDICE(G2:G5;C8)"),
        (12, 0, "Caixa de combinação: número do item escolhido"), (12, 2, "1"), (12, 3, "=ÍNDICE(H2:H4;C13)"),
        (14, 0, "Barra de rolagem: de 0 a 100, página de 10"), (14, 2, "50"), (14, 3, "=REPT(\"■\";C15/10)"),
        (16, 0, "Botões de opção: número do marcado"), (16, 2, "1"), (16, 3, "=ESCOLHER(C17;\"Pix\";\"Cartão\")"),
        (19, 0, "Botão: clique direito > Atribuir macro…"),
        (0, 6, "Produtos"), (1, 6, "Caneta"), (2, 6, "Caderno"), (3, 6, "Mochila"), (4, 6, "Estojo"),
        (0, 7, "Cores"), (1, 7, "Azul"), (2, 7, "Verde"), (3, 7, "Vermelho"),
    ];
    for &(r, c, t) in linhas {
        wb.escrever_celula(0, r, c, t.to_owned());
    }
    let mut lista = ObjetosAba::default();
    let mut c = ControleFormulario::novo(T::Girar);
    (c.celula, c.valor, c.min, c.max, c.passo) = (Some("$C$3".into()), Some(3), Some(1), Some(10), Some(1));
    lista.adicionar(controle(c, 2, 4, 20.0, None));
    let mut c = ControleFormulario::novo(T::Caixa);
    c.celula = Some("$C$6".into());
    lista.adicionar(controle(c, 5, 6, 130.0, Some("Cobrar frete")));
    let mut c = ControleFormulario::novo(T::Lista);
    (c.celula, c.faixa, c.valor) = (Some("$C$8".into()), Some("$G$2:$G$5".into()), Some(2));
    lista.adicionar(controle(c, 7, 11, 130.0, None));
    let mut c = ControleFormulario::novo(T::Combo);
    (c.celula, c.faixa, c.valor, c.linhas) = (Some("$C$13".into()), Some("$H$2:$H$4".into()), Some(1), Some(3));
    lista.adicionar(controle(c, 12, 13, 130.0, None));
    let mut c = ControleFormulario::novo(T::Rolagem);
    (c.celula, c.valor, c.min, c.max, c.passo, c.pagina, c.horizontal) = (Some("$C$15".into()), Some(50), Some(0), Some(100), Some(1), Some(10), true);
    lista.adicionar(controle(c, 14, 15, 130.0, None));
    for (i, nome) in ["Pix", "Cartão"].into_iter().enumerate() {
        let mut c = ControleFormulario::novo(T::Opcao);
        c.celula = Some("$C$17".into());
        c.marcado = Some(u8::from(i == 0));
        lista.adicionar(controle(c, 16 + i as u32, 17 + i as u32, 130.0, Some(nome)));
    }
    let mut b = controle(ControleFormulario::novo(T::Botao), 19, 21, 130.0, Some("Meu botão"));
    b.fonte = Some(feathersheet_io::objetos_aba::FonteTexto { negrito: true, ..Default::default() });
    lista.adicionar(b);
    let aba = wb.aba_mut(0).expect("aba 1");
    for (col, px) in [(0, 330), (1, 145), (2, 95), (3, 190)] {
        aba.sheet.col_widths.insert(col, px);
    }
    aba.sheet.ob = lista.gravar();
    let bytes = feathersheet_io::xlsx_write::gerar_xlsx(&wb).expect("gerar xlsx");
    std::fs::write(&saida, bytes).expect("gravar arquivo");
    println!("{saida}");
}
