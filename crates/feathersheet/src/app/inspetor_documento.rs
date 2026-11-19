//! Arquivo ▸ Informações ▸ Inspecionar documento, como o Inspetor de Documento do Excel: antes de
//! mandar a pasta para alguém, mostra o que vai junto sem estar à vista (comentários, dados pessoais,
//! abas, linhas e colunas ocultas, nomes, vínculos com outros arquivos, macros).
//!
//! Só remove o que volta com Ctrl+Z ou com um clique (comentários, dados pessoais); abas e linhas
//! ocultas são apontadas, não apagadas: apagar dado escondido é decisão de quem conhece a planilha.
//! A lista é montada no clique (não a cada quadro) e refeita depois de cada remoção.
use super::*;
use feathersheet_core::celulas::refs::{Rect, MAX_COLS, MAX_ROWS};

#[derive(Debug, Clone, PartialEq)]
pub enum Evento {
    Inspecionar,
    RemoverComentarios,
    RemoverDadosPessoais,
    GravarDadosPessoais,
    /// Fecha o menu e abre o Reexibir abas que já existe.
    ReexibirAbas,
}

/// Uma coisa achada: o que é, o detalhe e, quando há, o botão que resolve.
#[derive(Debug, Clone)]
pub struct Achado {
    pub titulo: String,
    pub detalhe: String,
    pub acao: Option<(String, Msg)>,
}

/// Nomes das abas, no máximo 5, com "e mais N".
fn lista_abas(nomes: &[String]) -> String {
    let mut texto = nomes.iter().take(5).cloned().collect::<Vec<_>>().join(", ");
    if nomes.len() > 5 { texto += &trf!(" e mais {}", nomes.len() - 5); }
    texto
}

impl App {
    pub(super) fn inspecionar_documento(&self) -> Vec<Achado> {
        let wb = &self.workbook;
        let abas: Vec<&feathersheet_core::planilha::sheet::Sheet> = (0..wb.len()).filter_map(|i| wb.aba(i)).map(|a| &a.sheet).collect();
        let mut achados = Vec::new();

        let com_comentario: Vec<String> = abas.iter().filter(|s| !s.comentarios.is_empty()).map(|s| s.name.clone()).collect();
        if !com_comentario.is_empty() {
            let total: usize = abas.iter().map(|s| s.comentarios.len()).sum();
            achados.push(Achado {
                titulo: trf!("Comentários: {}", total),
                detalhe: trf!("Com o nome de quem escreveu. Abas: {}", lista_abas(&com_comentario)),
                acao: Some((tr!("Remover todos").into(), Msg::InspetorDocumento(Evento::RemoverComentarios))),
            });
        }

        let p = &wb.propriedades;
        if p.sem_dados_pessoais {
            achados.push(Achado {
                titulo: tr!("Dados pessoais: não são gravados").into(),
                detalhe: tr!("Ao salvar, autor, \"modificado por\" e empresa ficam fora deste arquivo.").into(),
                acao: Some((tr!("Voltar a gravar").into(), Msg::InspetorDocumento(Evento::GravarDadosPessoais))),
            });
        } else {
            let campos: Vec<String> = [(tr!("Autor"), &p.autor), (tr!("Modificado por"), &p.modificado_por),
                (tr!("Empresa"), &p.organizacao), (tr!("Gerente"), &p.gerente)]
                .into_iter().filter(|(_, v)| !v.is_empty()).map(|(n, v)| format!("{n}: {v}")).collect();
            achados.push(Achado {
                titulo: tr!("Dados pessoais").into(),
                detalhe: if campos.is_empty() { tr!("Seu nome e empresa entram no arquivo ao salvar.").into() } else { campos.join(" · ") },
                acao: Some((tr!("Remover").into(), Msg::InspetorDocumento(Evento::RemoverDadosPessoais))),
            });
        }

        let ocultas: Vec<String> = wb.abas_ocultas().into_iter().filter_map(|i| wb.aba(i)).map(|a| a.sheet.name.clone()).collect();
        if !ocultas.is_empty() {
            achados.push(Achado {
                titulo: trf!("Abas ocultas: {}", ocultas.len()),
                detalhe: trf!("{}. Os dados delas vão no arquivo.", lista_abas(&ocultas)),
                acao: Some((tr!("Reexibir…").into(), Msg::InspetorDocumento(Evento::ReexibirAbas))),
            });
        }

        let com_ocultas: Vec<String> = abas.iter().filter(|s| s.tem_linhas_ocultas() || s.colunas_ocultas_desde.is_some()
            || s.col_widths.values().any(|&w| w == 0)).map(|s| s.name.clone()).collect();
        if !com_ocultas.is_empty() {
            achados.push(Achado {
                titulo: tr!("Linhas ou colunas ocultas").into(),
                detalhe: trf!("{}. Quem abrir o arquivo pode reexibir.", lista_abas(&com_ocultas)),
                acao: None,
            });
        }

        let nomes = wb.nomeados().len();
        if nomes > 0 {
            achados.push(Achado {
                titulo: trf!("Nomes definidos: {}", nomes),
                detalhe: tr!("Veja em Fórmulas ▸ Gerenciador de nomes.").into(),
                acao: None,
            });
        }

        let mut arquivos: Vec<String> = wb.areas_citadas().into_iter().map(|a| a.arquivo).collect();
        arquivos.sort();
        arquivos.dedup();
        if !arquivos.is_empty() {
            achados.push(Achado {
                titulo: trf!("Vínculos com outros arquivos: {}", arquivos.len()),
                detalhe: trf!("O nome e o caminho vão no arquivo: {}", arquivos.join(", ")),
                acao: None,
            });
        }

        let macros = wb.macros.as_deref().is_some_and(|m| !matches!(m.trim(), "" | "[]" | "{}"));
        if macros || wb.vba.is_some() {
            achados.push(Achado {
                titulo: tr!("Macros").into(),
                detalhe: tr!("A pasta leva código de macro. Só abra macros de quem você confia.").into(),
                acao: None,
            });
        }
        achados
    }

    pub(super) fn evento_inspetor_documento(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Inspecionar => {}
            Evento::ReexibirAbas => {
                let tarefa = self.update(Msg::FecharMenu);
                return Task::batch([tarefa, self.update(Msg::MostrarAbasOcultas)]);
            }
            Evento::RemoverComentarios => {
                let tudo = Rect { r0: 0, c0: 0, r1: MAX_ROWS - 1, c1: MAX_COLS - 1, sheet: None };
                let mut total = 0;
                for i in 0..self.workbook.len() {
                    if self.workbook.aba(i).is_some_and(|a| !a.sheet.comentarios.is_empty()) {
                        self.workbook.editar(i, |s| total += s.limpar_comentarios(&tudo));
                    }
                }
                if total > 0 {
                    self.alterado = true;
                    self.comentario_sobre = None;
                    // shortcut: um Ctrl+Z por aba (cada aba tem seu desfazer), não um para a pasta inteira.
                    self.status = trf!("{} comentários removidos. Ctrl+Z desfaz em cada aba.", total);
                }
            }
            Evento::RemoverDadosPessoais => {
                let p = &mut self.workbook.propriedades;
                p.autor.clear();
                p.modificado_por.clear();
                p.organizacao.clear();
                p.gerente.clear();
                p.sem_dados_pessoais = true;
                self.alterado = true;
                // shortcut: o nome de quem escreveu cada comentário fica; "Remover todos" acima tira os comentários.
                self.status = tr!("Dados pessoais removidos. Ao salvar, seu nome não entra neste arquivo.").into();
            }
            Evento::GravarDadosPessoais => {
                self.workbook.propriedades.sem_dados_pessoais = false;
                self.alterado = true;
                self.status = tr!("Seu nome e empresa voltam a entrar no arquivo ao salvar.").into();
            }
        }
        self.inspecao_documento = Some(self.inspecionar_documento());
        Task::none()
    }

    /// O bloco na página Informações: o botão e, depois do clique, o que foi achado.
    pub(super) fn vista_inspetor_documento(&self) -> Element<'_, Msg> {
        let negrito = pelicano::Font { weight: 600, ..pelicano::Font::DEFAULT };
        let mut lista = pelicano::column![
            text(tr!("Inspecionar documento")).size(15).font(negrito),
            text(tr!("Antes de mandar a pasta para alguém, veja o que vai junto sem estar à vista."))
                .size(11.5).color(COR_TEXTO_MUTED()),
            button(tr!("Inspecionar")).on_press(Msg::InspetorDocumento(Evento::Inspecionar)).style(estilo_botao_secundario),
        ].spacing(8);
        for achado in self.inspecao_documento.iter().flatten() {
            let mut linha = pelicano::row![
                pelicano::column![
                    text(achado.titulo.clone()).size(13).font(negrito),
                    text(achado.detalhe.clone()).size(11.5).color(COR_TEXTO_MUTED()),
                ].spacing(2).width(Length::Fixed(420.0)),
            ].spacing(12).align(pelicano::Align::Center);
            if let Some((rotulo, msg)) = &achado.acao {
                linha = linha.push(button(text(rotulo.clone()).size(12.5)).on_press(msg.clone()).style(estilo_botao_secundario));
            }
            lista = lista.push(linha);
        }
        lista.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use feathersheet_core::planilha::sheet::comentarios::Comentario;

    #[test]
    fn acha_comentario_e_aba_oculta_e_remove_dados_pessoais_sem_carimbar_de_novo() {
        let mut app = App::default();
        app.workbook.criar_aba("Segredo");
        app.workbook.editar(0, |s| { s.definir_comentario(2, 1, Some(Comentario { autor: "Ana".into(), texto: "ver".into(), visivel: false })); });
        app.workbook.ocultar_aba(1).unwrap();
        app.workbook.propriedades.autor = "Ana Lima".into();
        let _ = app.update(Msg::InspetorDocumento(Evento::Inspecionar));
        let titulos: Vec<String> = app.inspecao_documento.as_ref().unwrap().iter().map(|a| a.titulo.clone()).collect();
        assert!(titulos.iter().any(|t| t.contains('1') && t.starts_with(tr!("Comentários"))), "{titulos:?}");
        assert!(titulos.iter().any(|t| t.starts_with(tr!("Abas ocultas"))), "{titulos:?}");

        let _ = app.update(Msg::InspetorDocumento(Evento::RemoverComentarios));
        assert!(app.workbook.aba(0).unwrap().sheet.comentarios.is_empty());
        let _ = app.update(Msg::Desfazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.comentarios.len(), 1, "Ctrl+Z traz de volta");

        let _ = app.update(Msg::InspetorDocumento(Evento::RemoverDadosPessoais));
        app.carimbar_propriedades();
        let p = &app.workbook.propriedades;
        assert!(p.autor.is_empty() && p.modificado_por.is_empty() && p.organizacao.is_empty(), "{p:?}");
        assert!(!p.modificado.is_empty(), "a data continua");
        let _ = app.update(Msg::InspetorDocumento(Evento::GravarDadosPessoais));
        app.carimbar_propriedades();
        assert!(!app.workbook.propriedades.autor.is_empty());
        let _: Element<'_, Msg> = app.vista_inspetor_documento();
        app.menu_aberto = true;
        let _ = app.update(Msg::InspetorDocumento(Evento::ReexibirAbas));
        assert!(!app.menu_aberto && app.reexibir_abas_aberto);
    }
}
