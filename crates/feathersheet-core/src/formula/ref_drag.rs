//! Arrastar a referência de uma fórmula, como no Excel: pegar o retângulo
//! tracejado pela borda para mover, ou pela quina de baixo à direita para
//! redimensionar. Ao soltar, a fórmula é reescrita.
//!
//! Tradução do `core/ref-drag.ts`. Aqui fica só a parte que não precisa de
//! tela — achar as referências, decidir qual foi agarrada, a área nova e o
//! texto novo. O pixel é da `ui/grade/grid.ts`.
//!
//! A parte de texto (`reference_boxes`, `format_box` e a reescrita por
//! posição) foi portada antes, junto com o `assist.ts`, e mora em
//! [`super::assistente`]; este módulo a reexporta para ter o `ref-drag.ts`
//! inteiro num nome só.
//!
//! O que o texto tinha, o texto continua tendo: as travas (`$A$1`) ficam onde
//! estavam, e só o trecho da referência arrastada é trocado.

use crate::celulas::refs::{Rect, MAX_COLS, MAX_ROWS};

pub use super::assistente::{format_box, reference_boxes, Caixa, Travas};

/// Onde a pessoa pegou o retângulo (`GrabKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pegada {
    /// Pela borda: a área anda inteira.
    Mover,
    /// Por uma das quatro quinas: a área estica, a quina oposta fica.
    Redimensionar,
}

/// Qual referência está debaixo da célula, se alguma (`boxAt`). Vale a
/// última: quando duas se sobrepõem, a de cima é a que aparece depois na
/// fórmula. O TypeScript devolve -1 para "nenhuma"; aqui é `None`.
pub fn box_at(caixas: &[Caixa], linha: u32, coluna: u32) -> Option<usize> {
    caixas.iter().rposition(|c| c.rect.contains(linha, coluna))
}

/// `Math.max(0, Math.min(v, teto))`, em 64 bits para o deslocamento negativo
/// e para o teto que ficaria negativo não darem a volta.
fn prender(v: i64, teto: i64) -> u32 {
    v.min(teto).max(0) as u32
}

/// Desloca a área inteira, sem deixar sair da planilha nem mudar de tamanho
/// (`moveRect`). Como no TypeScript, o retângulo novo não leva a aba.
pub fn move_rect(rect: &Rect, d_linha: i64, d_coluna: i64) -> Rect {
    let altura = rect.r1 as i64 - rect.r0 as i64;
    let largura = rect.c1 as i64 - rect.c0 as i64;
    let r0 = prender(rect.r0 as i64 + d_linha, MAX_ROWS as i64 - 1 - altura);
    let c0 = prender(rect.c0 as i64 + d_coluna, MAX_COLS as i64 - 1 - largura);
    Rect::new(r0, c0, (r0 as i64 + altura) as u32, (c0 as i64 + largura) as u32)
}

/// Estica a área a partir da quina de cima à esquerda até a célula apontada
/// (`resizeRect`). Puxando para trás da âncora, o retângulo se vira.
pub fn resize_rect(rect: &Rect, linha: i64, coluna: i64) -> Rect {
    Rect::new(rect.r0, rect.c0, prender(linha, MAX_ROWS as i64 - 1), prender(coluna, MAX_COLS as i64 - 1))
        .normalized()
}

/// A área nova de um arrasto: mover pelo deslocamento de `de` até `alvo`, ou
/// esticar até `alvo`. É o que o `onMouseMove` da grade fazia com as duas
/// funções acima.
pub fn drag_rect(rect: &Rect, pegada: Pegada, de: (u32, u32), alvo: (u32, u32)) -> Rect {
    match pegada {
        Pegada::Mover => move_rect(rect, alvo.0 as i64 - de.0 as i64, alvo.1 as i64 - de.1 as i64),
        Pegada::Redimensionar => {
            // Qualquer quina estica (como no Excel): a quina oposta à pega
            // fica parada. Área de uma linha/coluna conta como pega embaixo/à direita.
            let de_cima = de.0 == rect.r0 && rect.r0 != rect.r1;
            let da_esquerda = de.1 == rect.c0 && rect.c0 != rect.c1;
            let fixa = Rect::cell(if de_cima { rect.r1 } else { rect.r0 }, if da_esquerda { rect.c1 } else { rect.c0 });
            resize_rect(&fixa, alvo.0 as i64, alvo.1 as i64)
        }
    }
}

/// A fórmula com aquela referência trocada pela área nova (`rewriteBox`).
pub fn rewrite_box(texto: &str, caixa: &Caixa, rect: &Rect) -> String {
    super::assistente::rewrite_box(texto, caixa.start, caixa.end, caixa.locks, rect)
}

/// As referências que valem para a aba à vista (`boxesForSheet`): as que
/// nomeiam essa aba, e as sem nome quando a fórmula está sendo escrita nela
/// mesma. É o que decide qual retângulo aparece e qual pode ser arrastado.
/// A comparação do nome ignora maiúscula (`toLowerCase` dos dois lados).
pub fn boxes_for_sheet<'a>(caixas: &'a [Caixa], aba: &str, na_aba_da_formula: bool) -> Vec<&'a Caixa> {
    let aba = aba.to_lowercase();
    caixas
        .iter()
        .filter(|c| match &c.sheet {
            None => na_aba_da_formula,
            Some(s) => s.to_lowercase() == aba,
        })
        .collect()
}

#[cfg(test)]
mod testes {
    //! Os casos do `test/core/ref-drag.test.ts`, um a um.
    use super::*;

    fn r(r0: u32, c0: u32, r1: u32, c1: u32) -> Rect {
        Rect::new(r0, c0, r1, c1)
    }

    fn sem_travas() -> Travas {
        Travas::default()
    }

    #[test]
    fn acha_cada_referencia_com_a_area_certa() {
        let caixas = reference_boxes("=SOMASE(B1:B8;C1:C8;D1:D8)");
        assert_eq!(caixas.len(), 3);
        assert_eq!(caixas[0].rect, r(0, 1, 7, 1));
        assert_eq!(caixas[2].rect, r(0, 3, 7, 3));
        assert_ne!(caixas[0].color, caixas[1].color, "cores diferentes");
    }

    #[test]
    fn a_referencia_diz_de_que_aba_e() {
        let t = "=Dados!A1+B2";
        let caixas = reference_boxes(t);
        assert_eq!(caixas.len(), 2);
        assert_eq!(caixas[0].sheet.as_deref(), Some("Dados"));
        assert_eq!(caixas[1].sheet, None, "sem prefixo é da própria aba");
        // O começo fica no A1: reescrever a área não pode comer o prefixo.
        let l: Vec<char> = t.chars().collect();
        assert_eq!(l[caixas[0].start..caixas[0].end].iter().collect::<String>(), "A1");
    }

    #[test]
    fn nome_de_aba_entre_aspas() {
        assert_eq!(reference_boxes("='Meu Plano'!B2*2")[0].sheet.as_deref(), Some("Meu Plano"));
        assert_eq!(reference_boxes("='Aba d''Água'!B2")[0].sheet.as_deref(), Some("Aba d'Água"));
    }

    #[test]
    fn cada_aba_mostra_so_os_retangulos_dela() {
        let caixas = reference_boxes("=SOMA(Dados!A1:A9;B2;Resumo!C3)");
        assert_eq!(caixas.len(), 3);

        let na_propria = boxes_for_sheet(&caixas, "Planilha1", true);
        assert_eq!(na_propria.iter().map(|c| c.sheet.clone()).collect::<Vec<_>>(), vec![None]);

        let em_dados = boxes_for_sheet(&caixas, "Dados", false);
        assert_eq!(em_dados.len(), 1);
        assert_eq!(em_dados[0].color, caixas[0].color, "a cor é a mesma nas duas abas");
        assert_eq!(boxes_for_sheet(&caixas, "dados", false).len(), 1, "maiúscula não conta");

        assert!(boxes_for_sheet(&caixas, "Outra", false).is_empty());
    }

    #[test]
    fn maiuscula_acentuada_tambem_nao_conta() {
        let caixas = reference_boxes("='Água'!A1");
        assert_eq!(boxes_for_sheet(&caixas, "ÁGUA", false).len(), 1);
    }

    #[test]
    fn arrastar_de_outra_aba_mantem_o_prefixo() {
        let t = "=SOMA(Dados!A1:A9)";
        let caixa = &reference_boxes(t)[0];
        assert_eq!(rewrite_box(t, caixa, &r(0, 1, 8, 1)), "=SOMA(Dados!B1:B9)");
    }

    #[test]
    fn texto_entre_aspas_nao_vira_referencia() {
        assert_eq!(reference_boxes("=\"A1 não conta\"&B2").len(), 1);
    }

    #[test]
    fn a_celula_debaixo_do_ponteiro_diz_qual_foi_pega() {
        let caixas = reference_boxes("=SOMA(A1:B2;D1:D9)");
        assert_eq!(box_at(&caixas, 0, 0), Some(0));
        assert_eq!(box_at(&caixas, 5, 3), Some(1));
        assert_eq!(box_at(&caixas, 5, 5), None, "fora de todas");
    }

    #[test]
    fn quando_duas_se_sobrepoem_vale_a_de_cima() {
        let caixas = reference_boxes("=SOMA(A1:C3;B2:D4)");
        assert_eq!(box_at(&caixas, 1, 1), Some(1), "a última da fórmula é a que está por cima");
    }

    #[test]
    fn mover_mantem_o_tamanho_e_nao_sai_da_planilha() {
        assert_eq!(move_rect(&r(2, 2, 4, 3), 3, 1), r(5, 3, 7, 4));
        assert_eq!(move_rect(&r(0, 0, 1, 1), -5, -5), r(0, 0, 1, 1));

        let no_fim = move_rect(&r(0, 0, 2, 2), MAX_ROWS as i64, MAX_COLS as i64);
        assert_eq!(no_fim.r1, MAX_ROWS - 1, "encosta no fim sem encolher");
        assert_eq!(no_fim.c1, MAX_COLS - 1);
        assert_eq!(no_fim.r1 - no_fim.r0, 2, "e sem mudar de tamanho");
    }

    #[test]
    fn mover_nao_leva_a_aba_do_retangulo() {
        // Como o `{ r0, c0, r1, c1 }` do TypeScript: o retângulo novo sai sem
        // `sheet`. Quem sabe a aba é a caixa, não a área.
        let com_aba = Rect::na_aba(0, 0, 1, 1, "Dados");
        assert_eq!(move_rect(&com_aba, 1, 1).sheet, None);
        assert_eq!(resize_rect(&com_aba, 3, 3).sheet, None);
    }

    #[test]
    fn redimensionar_estica_a_partir_da_quina_de_cima() {
        assert_eq!(resize_rect(&r(1, 1, 2, 2), 9, 5), r(1, 1, 9, 5));
        // Puxando para trás da âncora, o retângulo se vira.
        assert_eq!(resize_rect(&r(3, 3, 4, 4), 0, 0), r(0, 0, 3, 3));
        // Fora da planilha, encosta na borda.
        assert_eq!(resize_rect(&r(3, 3, 4, 4), -2, i64::from(MAX_COLS) + 9), r(0, 3, 3, MAX_COLS - 1));
    }

    #[test]
    fn a_formula_e_reescrita_so_no_pedaco_arrastado() {
        let t = "=SOMASE(B1:B8;C1:C8;D1:D8)";
        let caixas = reference_boxes(t);
        assert_eq!(rewrite_box(t, &caixas[1], &r(0, 2, 19, 2)), "=SOMASE(B1:B8;C1:C20;D1:D8)");
    }

    #[test]
    fn as_travas_sobrevivem_ao_arrasto() {
        let t = "=SOMA($B$1:$B$8)";
        let caixas = reference_boxes(t);
        assert_eq!(caixas[0].locks, Travas { c0: true, r0: true, c1: true, r1: true });
        assert_eq!(rewrite_box(t, &caixas[0], &r(2, 2, 9, 2)), "=SOMA($C$3:$C$10)");

        let misto = "=SOMA($B1:B$8)";
        let caixas = reference_boxes(misto);
        assert_eq!(rewrite_box(misto, &caixas[0], &r(0, 2, 7, 2)), "=SOMA($C1:C$8)");
    }

    #[test]
    fn uma_celula_vira_intervalo_ao_ser_esticada_e_volta() {
        assert_eq!(format_box(&r(0, 0, 0, 0), sem_travas()), "A1");
        let t = "=A1*2";
        let caixas = reference_boxes(t);
        assert_eq!(rewrite_box(t, &caixas[0], &r(0, 0, 4, 0)), "=A1:A5*2");
    }

    #[test]
    fn arrastar_duas_vezes_parte_sempre_do_texto_certo() {
        // Cada movimento do mouse reescreve a partir do texto de quando o
        // botão desceu.
        let original = "=MÉDIA(C3:D4)";
        let caixa = &reference_boxes(original)[0];
        let um = rewrite_box(original, caixa, &move_rect(&caixa.rect, 1, 0));
        let dois = rewrite_box(original, caixa, &move_rect(&caixa.rect, 2, 0));
        assert_eq!(um, "=MÉDIA(C4:D5)");
        assert_eq!(dois, "=MÉDIA(C5:D6)");
    }

    #[test]
    fn qualquer_quina_estica_com_a_oposta_parada() {
        let r = Rect::new(2, 2, 5, 4); // C3:E6
        // Quina de cima à esquerda puxada para A1: a de baixo à direita fica.
        assert_eq!(drag_rect(&r, Pegada::Redimensionar, (2, 2), (0, 0)), Rect::new(0, 0, 5, 4));
        // De cima à direita, para a direita e para baixo de nada: só a coluna cresce.
        assert_eq!(drag_rect(&r, Pegada::Redimensionar, (2, 4), (2, 6)), Rect::new(2, 2, 5, 6));
        // De baixo à esquerda, para cima: encolhe.
        assert_eq!(drag_rect(&r, Pegada::Redimensionar, (5, 2), (3, 2)), Rect::new(2, 2, 3, 4));
        // De baixo à direita: como sempre foi.
        assert_eq!(drag_rect(&r, Pegada::Redimensionar, (5, 4), (7, 5)), Rect::new(2, 2, 7, 5));
    }

    #[test]
    fn drag_rect_e_o_que_a_grade_fazia() {
        let rect = r(2, 2, 4, 3);
        assert_eq!(drag_rect(&rect, Pegada::Mover, (3, 2), (6, 3)), move_rect(&rect, 3, 1));
        assert_eq!(drag_rect(&rect, Pegada::Mover, (6, 3), (3, 2)), move_rect(&rect, -3, -1));
        assert_eq!(drag_rect(&rect, Pegada::Redimensionar, (4, 3), (9, 5)), resize_rect(&rect, 9, 5));
    }

    #[test]
    fn a_trava_de_uma_celula_so_vale_para_as_duas_pontas() {
        // Esquisitice preservada do TypeScript: `$A1` esticado vira `$A1:$A5`,
        // porque a ponta que não existia copia as travas da única que havia
        // (`mb = b === undefined ? ma : …`). Fica anotado aqui em vez de
        // consertado: portar é traduzir.
        let t = "=$A1";
        let caixas = reference_boxes(t);
        assert_eq!(rewrite_box(t, &caixas[0], &r(0, 0, 4, 0)), "=$A1:$A5");
    }
}
