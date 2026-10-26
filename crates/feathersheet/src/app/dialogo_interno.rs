//! Diálogos internos do FeatherSheet sobre o componente comum
//! `feather_ui::dialogo`: posição logo abaixo da faixa, teto pela janela.
use super::*;
pub(super) use feather_ui::dialogo::{self as dlg, Dialogo};

impl App {
    /// Onde o diálogo começa: logo abaixo da faixa (ou das abas, se recolhida).
    pub(super) fn topo_dialogo(&self) -> f32 {
        43.0 + 52.0
            + if self.config.faixa_recolhida { 0.0 } else { faixa_nativa::ALTURA_FAIXA_NATIVA }
            + 8.0
    }

    /// Diálogo já posicionado e limitado ao tamanho da janela principal.
    pub(super) fn dialogo<'a>(
        &self,
        titulo: impl Into<String>,
        corpo: impl Into<Element<'a, Msg>>,
    ) -> Dialogo<'a, Msg> {
        let mut d = Dialogo::new(titulo, corpo).topo(self.topo_dialogo());
        // Antes do primeiro desenho o tamanho ainda é zero: fica o teto padrão.
        if self.tamanho_principal.height > 0.0 {
            d = d.altura_janela(self.tamanho_principal.height);
        }
        if self.tamanho_principal.width > 0.0 {
            d = d.largura_janela(self.tamanho_principal.width);
        }
        d
    }

    /// `FEATHERSHEET_CAPTURAR_DIALOGO=nomes|nomes_novo|erro|estatisticas|perfil|link|…`:
    /// abre o diálogo para a foto do modo de captura.
    pub(super) fn preparar_captura_dialogo(&mut self, nome: &str) {
        let _ = match nome {
            "nomes" => self.update(Msg::NomesAbrir(false)),
            "aviso_xlsx" => {
                self.aviso_xlsx = Some(vec!["CONT.ÚNICO".into(), "IMPORTARJSON".into()]);
                Task::none()
            }
            "nomes_novo" => self.update(Msg::NomesAbrir(true)),
            // Três nomes de exemplo, o segundo escolhido.
            "nomes_exemplo" => {
                let aba = self.nome_aba.clone();
                self.workbook.definir_nome("Receitas", Rect::na_aba(1, 1, 12, 1, aba.clone()));
                self.workbook.definir_nome("Taxa", Rect::na_aba(0, 4, 0, 4, aba.clone()));
                self.workbook.definir_nome("Despesas", Rect::na_aba(1, 2, 12, 2, aba));
                let _ = self.update(Msg::NomesAbrir(false));
                self.update(Msg::NomesEvento(nomes_ui::Evento::Selecionar("Taxa".into())))
            }
            "erro" => {
                self.erro_explicando = Some((self.folha, self.linha, self.coluna));
                Task::none()
            }
            "estatisticas" => self.update(Msg::EstatisticasAbrir),
            "perfil" => self.update(Msg::PerfilColunaAbrir),
            "link" => self.update(Msg::LinkEditar),
            "cenarios" => self.update(Msg::Cenarios(cenarios_ui::Evento::Abrir)),
            "ir_para" => self.update(Msg::IrParaJanela(ir_para::Evento::Abrir)),
            "colar_especial" => {
                let _ = self.update(Msg::Copiar);
                self.update(Msg::ColarEspecialJanela(colar_especial::Evento::Abrir))
            }
            "classificar" => self.update(Msg::Classificar(classificar::Evento::Abrir)),
            "atingir_meta" => self.update(Msg::AtingirMeta(atingir_meta::Evento::Abrir)),
            "tabela_dados" => self.update(Msg::TabelaDados(tabela_dados::Evento::Abrir)),
            "avaliar" => self.update(Msg::Avaliar(avaliar_ui::Evento::Abrir)),
            "celulas" => self.update(Msg::Celulas(celulas_dialogo::Evento::Abrir(true))),
            "dimensao" => self.update(Msg::Dimensao(dimensao_dialogo::Evento::Abrir(true))),
            "protecao" => self.update(Msg::Protecao(protecao_ui::Evento::Abrir)),
            "subtotal" => self.update(Msg::Topicos(topicos_ui::Evento::SubtotalAbrir)),
            "novo_estilo" => self.update(Msg::NovoEstilo(estilo_novo::Evento::Abrir)),
            // Série… e Validação de dados (abas, alerta, balão, círculos).
            "serie" | "serie_data" => self.preparar_captura_serie(nome),
            n if n.starts_with("validacao") => self.preparar_captura_validacao(n),
            "macro" => {
                self.macro_mensagens.push(tr!("Relatório gerado com 12 linhas.").into());
                Task::none()
            }
            _ => Task::none(),
        };
    }
}
