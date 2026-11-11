//! DIN-6: "Agrupar seleção" e "Desagrupar" da aba Tabela Dinâmica, como no Excel: os itens
//! de Linhas (ou, no cabeçalho, de Colunas) selecionados na grade viram um item só
//! ("Grupo1"), que soma todos. O motor
//! guarda em `Extras::grupos_de_itens` (vale pela coluna da origem).
use super::*;
use feathersheet_core::dinamica::dinamica_layout::SEPARADOR;
use feathersheet_core::dinamica::exibicao::GrupoDeItens;

impl App {
    pub(super) fn agrupar_itens_selecionados(&mut self, agrupar: bool) {
        let area = match edicao::localizar_selecionada(self).map(|(_, _, meta)| retangulo_json(&meta["areaResultado"])) {
            Ok(Some(area)) => area,
            Ok(None) => return,
            Err(erro) => { self.status = erro; return }
        };
        let Some(e) = self.dinamica.as_ref() else { return };
        let resultado = match avancado::calcular(self, e) {
            Ok(r) => r,
            Err(erro) => { self.status = erro; return }
        };
        // Os itens das linhas selecionadas, pelo caminho de cada linha do resultado. No
        // tabular cada campo tem a sua coluna de rótulo: o nível é o da coluna selecionada
        // (como no Excel); no compacto, uma coluna só, é o da linha.
        let sel = self.area_selecionada();
        let coluna_de_rotulo = (resultado.colunas_de_rotulo > 1)
            .then(|| sel.c0.saturating_sub(area.c0) as usize)
            .filter(|&c| c < resultado.colunas_de_rotulo);
        let (mut nivel, mut itens) = (None, Vec::<String>::new());
        for r in sel.r0.max(area.r0)..=sel.r1.min(area.r1) {
            let Some(Some(caminho)) = resultado.caminhos.get((r - area.r0) as usize) else { continue };
            if caminho.is_empty() { continue; } // Total Geral
            let partes: Vec<&str> = caminho.split(SEPARADOR).collect();
            let d = coluna_de_rotulo.unwrap_or(partes.len() - 1);
            if d >= partes.len() { continue; } // subtotal de um nível acima

            if *nivel.get_or_insert(d) != d {
                self.status = tr!("Selecione itens do mesmo nível da dinâmica.").into();
                return;
            }
            if !itens.iter().any(|i| i == partes[d]) { itens.push(partes[d].to_owned()); }
        }
        // Seleção só no cabeçalho: os itens do campo de Colunas, pela chave de cada coluna
        // (com dois campos em Colunas a chave junta os níveis; vale o último).
        let no_cabecalho = (sel.r0.max(area.r0)..=sel.r1.min(area.r1))
            .all(|r| matches!(resultado.caminhos.get((r - area.r0) as usize), Some(None)));
        let mut eixo_colunas = false;
        if nivel.is_none() && no_cabecalho {
            for c in sel.c0.max(area.c0)..=sel.c1.min(area.c1) {
                let Some(Some(chave)) = resultado.chaves_colunas.get((c - area.c0) as usize) else { continue };
                if chave.is_empty() { continue; } // Total Geral
                let partes: Vec<&str> = chave.split(SEPARADOR).collect();
                let d = partes.len() - 1;
                nivel.get_or_insert(d);
                if !itens.iter().any(|i| i == partes[d]) { itens.push(partes[d].to_owned()); }
            }
            eixo_colunas = nivel.is_some();
        }
        let (Some(d), Some(e)) = (nivel, self.dinamica.as_mut()) else {
            self.status = tr!("Selecione os itens de Linhas ou de Colunas que quer agrupar.").into();
            return;
        };
        let campos = if eixo_colunas { &e.colunas } else { &e.linhas };
        let Some(&coluna) = campos.get(d) else { return };
        let extras = config_campo::extras_editaveis(e);
        let grupos = &mut extras.grupos_de_itens;
        if agrupar {
            // Um grupo selecionado entra com os itens dele (como o Excel junta grupos).
            let mut juntos = Vec::new();
            for item in itens {
                match grupos.iter().position(|g| g.coluna == coluna && g.nome == item) {
                    Some(i) => juntos.extend(grupos.remove(i).itens),
                    None => juntos.push(item),
                }
            }
            let nome = (1..).map(|n| trf!("Grupo{n}"))
                .find(|n| !grupos.iter().any(|g| g.coluna == coluna && &g.nome == n))
                .expect("um nome livre sempre existe");
            self.status = trf!("{} itens agrupados em {nome}.", juntos.len());
            grupos.push(GrupoDeItens { coluna, nome, itens: juntos });
        } else {
            let antes = grupos.len();
            grupos.retain(|g| !(g.coluna == coluna && itens.contains(&g.nome)));
            self.status = if grupos.len() < antes { tr!("Grupo desfeito.") } else { tr!("A seleção não tem grupo feito à mão.") }.into();
        }
    }

    /// Digitar por cima do rótulo de um grupo feito à mão renomeia o grupo, como no Excel.
    /// `true` quando era um grupo: quem confirma a edição não grava a célula.
    pub(in crate::app) fn renomear_grupo_na_celula(&mut self, folha: usize, linha: u32, coluna: u32, antes: &str, novo: &str) -> bool {
        let novo = novo.trim();
        if novo.is_empty() || novo.starts_with('=') || antes == novo || folha != self.folha
            || (linha, coluna) != (self.linha, self.coluna) { return false; }
        let Some((_, meta)) = edicao::localizar_na_celula(self, folha, linha, coluna) else { return false };
        let extras: Extras = serde_json::from_value(meta["extras"].clone()).unwrap_or_default();
        if !extras.grupos_de_itens.iter().any(|g| g.nome == antes) { return false; }
        let _ = self.evento_dinamica(Evento::NaFaixa(Box::new(Evento::RenomearGrupo(antes.to_owned(), novo.to_owned()))));
        true
    }

    /// O nome de um grupo trocado; recusa se o campo já tem um grupo com o nome novo.
    pub(super) fn renomear_grupo(&mut self, antes: &str, novo: &str) {
        let Some(e) = self.dinamica.as_mut() else { return };
        let campos: Vec<usize> = e.linhas.iter().chain(&e.colunas).copied().collect();
        let grupos = &mut config_campo::extras_editaveis(e).grupos_de_itens;
        let Some(i) = grupos.iter().position(|g| g.nome == antes && campos.contains(&g.coluna)) else { return };
        let coluna = grupos[i].coluna;
        if grupos.iter().any(|g| g.coluna == coluna && g.nome == novo) {
            self.status = trf!("Já existe um grupo chamado {novo}.");
            return;
        }
        grupos[i].nome = novo.to_owned();
        self.status = trf!("Grupo renomeado para {novo}.");
    }
}
