//! Manter a célula ativa à vista com a geometria REAL da grade: tamanho do
//! Canvas, zoom, alturas/larguras próprias, ocultas e painéis congelados.
//! Antes a conta usava 28 linhas × 12 colunas fixas da configuração; com
//! zoom abaixo de 100% cabiam mais, e clicar na borda de baixo ou da direita
//! rolava a tela e jogava a célula para o topo.
use super::*;
use super::congelado::Faixa;

/// O que conta como "à vista" para não rolar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Exigencia {
    /// Teclado, Ir para, Localizar…: a célula precisa caber inteira.
    Inteira,
    /// Clique do mouse: qualquer pedaço desenhado basta (nunca rola).
    Parcial,
}

/// Uma direção da grade (linhas ou colunas), já em pixels lógicos.
struct Direcao {
    faixas: Vec<Faixa>,
    /// Fim da parte fixa (cabeçalho + congeladas).
    divisor: f32,
    /// Onde a área útil acaba: a barra de rolagem cobre o resto.
    limite: f32,
    congeladas: u32,
    /// Primeira rolável desenhada (origem normalizada).
    origem: u32,
}

impl Direcao {
    /// `None`: a célula está à vista e a origem fica como está.
    fn nova_origem(&self, indice: u32, exigencia: Exigencia, eixo: &super::eixos::Eixo) -> Option<u32> {
        if indice < self.congeladas { return None; }
        let tamanho = eixo.tamanho(indice);
        if let Some(&(_, inicio, desenhado)) = self.faixas.iter().find(|f| f.0 == indice) {
            let a_vista = match exigencia {
                Exigencia::Parcial => inicio < self.limite,
                // Meio pixel de folga para arredondamento do zoom.
                Exigencia::Inteira => desenhado + 0.5 >= tamanho && inicio + tamanho <= self.limite + 0.5,
            };
            if a_vista { return None; }
        }
        if indice < self.origem { return Some(indice); }
        // Abaixo/à direita: a célula vira a última inteira da tela, como no
        // Excel (seta para baixo na última visível rola uma linha só).
        let disponivel = (self.limite - self.divisor).max(0.0);
        let (mut origem, mut ocupado) = (indice, tamanho);
        while origem > self.congeladas {
            let Some(anterior) = eixo.anterior_visivel(origem - 1)
                .filter(|a| *a >= self.congeladas) else { break };
            let soma = ocupado + eixo.tamanho(anterior);
            if soma > disponivel + 0.5 { break; }
            origem = anterior;
            ocupado = soma;
        }
        Some(origem)
    }

    /// Quantas roláveis cabem inteiras (passo do Page Down).
    fn inteiras(&self, eixo: &super::eixos::Eixo) -> u32 {
        self.faixas.iter()
            .filter(|&&(i, inicio, desenhado)| i >= self.congeladas
                && desenhado + 0.5 >= eixo.tamanho(i)
                && inicio + eixo.tamanho(i) <= self.limite + 0.5)
            .count() as u32
    }
}

impl Grade<'_> {
    fn direcoes(&self, tela: Size) -> (Direcao, Direcao) {
        let zoom = self.zoom.max(0.01);
        let (linhas, divisor_y) = self.linhas_visiveis(tela.height / zoom);
        let (colunas, divisor_x) = self.colunas_visiveis(tela.width / zoom);
        // As barras têm espessura fixa em pixels de TELA (não acompanham o zoom).
        let limite_y = (tela.height - barras::ESPESSURA) / zoom;
        let limite_x = (tela.width - barras::ESPESSURA) / zoom;
        (
            Direcao {
                faixas: linhas, divisor: divisor_y, limite: limite_y,
                congeladas: self.congelar_linhas,
                origem: self.primeira_linha_visivel(tela.height),
            },
            Direcao {
                faixas: colunas, divisor: divisor_x, limite: limite_x,
                congeladas: self.congelar_colunas,
                origem: self.primeira_coluna_visivel(tela.width),
            },
        )
    }

    /// Nova origem (linha, coluna) para `(r, c)` ficar à vista no Canvas de
    /// tamanho `tela` (pixels de tela). `None` em cada eixo: não rolar.
    pub(crate) fn origem_para_mostrar(
        &self, r: u32, c: u32, tela: Size, exigencia: Exigencia,
    ) -> (Option<u32>, Option<u32>) {
        let (vertical, horizontal) = self.direcoes(tela);
        let mut cache = self.eixos.borrow_mut();
        let Some((linhas, colunas)) = cache.para(
            self.workbook, self.aba, MAX_ROWS, MAX_COLS, ALTURA_PADRAO, LARGURA_PADRAO,
        ) else { return (None, None) };
        (vertical.nova_origem(r, exigencia, linhas),
         horizontal.nova_origem(c, exigencia, colunas))
    }

    /// Linhas e colunas roláveis que cabem inteiras na tela (mínimo 1).
    pub(crate) fn pagina(&self, tela: Size) -> (u32, u32) {
        let (vertical, horizontal) = self.direcoes(tela);
        let mut cache = self.eixos.borrow_mut();
        let Some((linhas, colunas)) = cache.para(
            self.workbook, self.aba, MAX_ROWS, MAX_COLS, ALTURA_PADRAO, LARGURA_PADRAO,
        ) else { return (1, 1) };
        (vertical.inteiras(linhas).max(1), horizontal.inteiras(colunas).max(1))
    }
}
