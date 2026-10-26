//! Geometria de painéis congelados, partilhada pelo desenho e hit-test.
//! O índice esparso veio de axis.ts; mesmo uma sequência de milhões de
//! linhas ocultas não causa um laço de milhões de iterações no Canvas.
use super::*;
use super::eixos::Eixo;

pub(super) type Faixa = (u32, f32, f32); // índice, início e extensão desenhada

/// Monta células fixas à esquerda/acima e células roláveis em seguida.
/// A borda fixa não pode consumir mais que 70% do viewport: o restante
/// deve continuar utilizável em telas pequenas e zoom alto.
fn faixas(
    tamanho: f32, cabecalho: f32, congeladas: u32, origem: u32, limite: u32,
    eixo: &Eixo, corte: f32,
) -> (Vec<Faixa>, f32) {
    let mut resultado = Vec::new();
    let mut posicao = cabecalho;
    let limite_congelado = cabecalho + (tamanho - cabecalho).max(0.0) * 0.70;
    let mut proximo = eixo.proximo_visivel(0);
    while let Some(indice) = proximo {
        if indice >= congeladas.min(limite) || posicao >= limite_congelado { break; }
        let desenhado = eixo.tamanho(indice).min((limite_congelado - posicao).max(0.0));
        if desenhado > 0.0 {
            resultado.push((indice, posicao, desenhado));
            posicao += desenhado;
        }
        proximo = eixo.proximo_visivel(indice + 1);
    }
    let divisor = posicao;

    let mut proximo = eixo.proximo_visivel(origem.max(congeladas).min(limite));
    // A primeira rolável pode aparecer cortada (fim da planilha, ver `corte_da_origem`).
    let mut corte = corte.max(0.0);
    while let Some(indice) = proximo {
        if indice >= limite || posicao >= tamanho { break; }
        let desenhado = (eixo.tamanho(indice) - std::mem::take(&mut corte)).max(0.0)
            .min((tamanho - posicao).max(0.0));
        if desenhado > 0.0 {
            resultado.push((indice, posicao, desenhado));
            posicao += desenhado;
        }
        proximo = eixo.proximo_visivel(indice + 1);
    }
    (resultado, divisor)
}

/// Tamanho inteiro de cada faixa. A última, cortada pela borda útil (`fim`),
/// volta ao tamanho real: o texto e o número do cabeçalho ficam onde estariam
/// com a linha inteira e só são recortados, como no Excel. Antes o texto era
/// centrado no pedaço visível (2 px), espremido e por cima da linha anterior.
pub(super) fn extensoes_inteiras(faixas: &[Faixa], fim: f32, eixo: &Eixo) -> Vec<f32> {
    faixas.iter().map(|&(indice, inicio, desenhado)| {
        if inicio + desenhado + 0.5 >= fim { desenhado.max(eixo.tamanho(indice)) } else { desenhado }
    }).collect()
}

impl Grade<'_> {
    /// Extensão lógica onde há células: as barras de rolagem (13 px de TELA)
    /// ficam com faixa própria e não cobrem a última linha/coluna.
    pub(super) fn area_util(&self, total: f32) -> f32 {
        (total - super::barras::ESPESSURA / self.zoom.max(0.01)).max(0.0)
    }

    pub(super) fn colunas_visiveis(&self, largura_total: f32) -> (Vec<Faixa>, f32) {
        let origem = self.primeira_coluna_visivel(largura_total * self.zoom);
        let corte = self.corte_da_origem(largura_total * self.zoom, origem);
        let mut cache = self.eixos.borrow_mut();
        let Some((_, colunas)) = cache.para(
            self.workbook, self.aba, MAX_ROWS, MAX_COLS,
            ALTURA_PADRAO, LARGURA_PADRAO,
        ) else { return (Vec::new(), CABECALHO_LINHA); };
        faixas(self.area_util(largura_total), CABECALHO_LINHA, self.congelar_colunas,
            origem, MAX_COLS, colunas, corte)
    }

    pub(super) fn linhas_visiveis(&self, altura_total: f32) -> (Vec<Faixa>, f32) {
        let origem = self.primeira_linha_visivel(altura_total * self.zoom);
        let mut cache = self.eixos.borrow_mut();
        let Some((linhas, _)) = cache.para(
            self.workbook, self.aba, MAX_ROWS, MAX_COLS,
            ALTURA_PADRAO, LARGURA_PADRAO,
        ) else { return (Vec::new(), CABECALHO_COLUNA); };
        faixas(self.area_util(altura_total), CABECALHO_COLUNA, self.congelar_linhas,
            origem, MAX_ROWS, linhas, 0.0)
    }

    /// Alturas e larguras inteiras das faixas desenhadas (ver [`extensoes_inteiras`]).
    /// `tela` em pixels lógicos, a mesma passada a `linhas_visiveis`/`colunas_visiveis`.
    pub(super) fn extensoes_inteiras(
        &self, linhas: &[Faixa], colunas: &[Faixa], tela: Size,
    ) -> (Vec<f32>, Vec<f32>) {
        let (fim_y, fim_x) = (self.area_util(tela.height), self.area_util(tela.width));
        let mut cache = self.eixos.borrow_mut();
        match cache.para(self.workbook, self.aba, MAX_ROWS, MAX_COLS, ALTURA_PADRAO, LARGURA_PADRAO) {
            Some((eixo_l, eixo_c)) => (extensoes_inteiras(linhas, fim_y, eixo_l),
                extensoes_inteiras(colunas, fim_x, eixo_c)),
            None => (linhas.iter().map(|f| f.2).collect(), colunas.iter().map(|f| f.2).collect()),
        }
    }

    pub(super) fn posicao_da_celula(&self, r: u32, c: u32, tamanho: Size)
        -> Option<(f32, f32, f32, f32)>
    {
        let (colunas, _) = self.colunas_visiveis(tamanho.width);
        let (linhas, _) = self.linhas_visiveis(tamanho.height);
        let (_, x, w) = colunas.into_iter().find(|(i, _, _)| *i == c)?;
        let (_, y, h) = linhas.into_iter().find(|(i, _, _)| *i == r)?;
        Some((x,y,w,h))
    }

    pub(super) fn ponto_para_celula(&self, p: Point, tamanho: Size) -> Option<(u32,u32)> {
        let (colunas, _) = self.colunas_visiveis(tamanho.width);
        let (linhas, _) = self.linhas_visiveis(tamanho.height);
        let c = colunas.iter().find(|(_,x,w)| p.x >= *x && p.x < *x+*w)?.0;
        let r = linhas.iter().find(|(_,y,h)| p.y >= *y && p.y < *y+*h)?.0;
        Some((r,c))
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn faixas_congeladas_sao_fixas_e_sem_copia_do_intervalo_rolado() {
        let eixo = Eixo::novo(10_000_000, 100., &HashMap::new(), None);
        let (itens, div) = faixas(580., 90., 2, 1_000_000, 10_000_000, &eixo, 0.0);
        assert_eq!(div, 290.);
        assert_eq!(itens.iter().map(|v|v.0).collect::<Vec<_>>(),
            vec![0,1,1_000_000,1_000_001,1_000_002]);
    }
    #[test]
    fn faixa_rolavel_nunca_repete_o_congelado() {
        let eixo = Eixo::novo(10_000_000, 28., &HashMap::new(), None);
        let (itens, _) = faixas(250., 30., 2, 0, 10_000_000, &eixo, 0.0);
        let linhas: Vec<u32> = itens.into_iter().map(|v| v.0).collect();
        assert_eq!(linhas[..2], [0,1]);
        assert_eq!(linhas[2], 2);
    }
    #[test]
    fn faixas_com_larguras_variaveis_e_ocultas_preservam_indices() {
        let medidas = HashMap::from([(0, 75), (1, 0)]);
        let eixo = Eixo::novo(1_000_000, 100., &medidas, None);
        let (itens, div) = faixas(500., 90., 2, 15, 1_000_000, &eixo, 0.0);
        assert_eq!(div,165.);
        let ids: Vec<u32> = itens.into_iter().map(|v|v.0).collect();
        assert_eq!(ids,vec![0,15,16,17,18]);
    }
    #[test]
    fn viewport_pequeno_nao_congela_a_tela_inteira() {
        let eixo = Eixo::novo(10_000_000, 100., &HashMap::new(), None);
        let (itens, div) = faixas(250., 90., 200, 30, 10_000_000, &eixo, 0.0);
        assert!(div < 250.);
        assert!(itens.len() <= 3);
    }
    #[test]
    fn ocultar_cem_mil_linhas_nao_pula_indevidamente_o_primeiro_valor() {
        let medidas = (0..100_000).map(|r| (r, 0)).collect::<HashMap<u32, u32>>();
        let eixo = Eixo::novo(10_000_000, 28., &medidas, None);
        let (itens, div) = faixas(200., 30., 0, 0, 10_000_000, &eixo, 0.0);
        assert_eq!(div, 30.);
        assert_eq!(itens.first().map(|v| v.0), Some(100_000));
        assert_eq!(itens.get(1).map(|v| v.0), Some(100_001));
    }
    #[test]
    fn so_a_faixa_cortada_no_fim_volta_ao_tamanho_inteiro() {
        let eixo = Eixo::novo(1_000, 18., &HashMap::new(), None);
        // 30 + 3 × 18 = 84; fim útil em 90: a quarta linha aparece com 6 px.
        let (itens, _) = faixas(90., 30., 0, 0, 1_000, &eixo, 0.0);
        assert_eq!(itens.last().map(|f| f.2), Some(6.0));
        let inteiras = extensoes_inteiras(&itens, 90., &eixo);
        assert_eq!(inteiras, vec![18., 18., 18., 18.]);
        // A coluna de origem cortada no começo (`corte`) não muda.
        let (itens, _) = faixas(500., 30., 0, 5, 1_000, &eixo, 10.0);
        assert_eq!(extensoes_inteiras(&itens, 500., &eixo)[0], 8.0);
    }
}
