//! APP-012: visualização de qualidade de uma coluna, sem modificar o workbook.
//! O algoritmo, quartis e distribuição pertencem ao feathersheet-core;
//! aqui só ficam acionamento, escopo selecionado e renderização Iced.
use super::*;
use feathersheet_core::operacoes::perfil::{self, PerfilColuna};

#[derive(Debug, Clone)]
pub struct PerfilVisao {
    pub(super) coluna: u32,
    area: Rect,
    inteira: bool,
    pub(super) dados: PerfilColuna,
}

fn numero(n: f64) -> String {
    if !n.is_finite() { return n.to_string(); }
    let s = format!("{n:.2}");
    s.replace('.', ",")
}

fn quantidade(n: usize) -> String {
    let s = n.to_string();
    let mut buffer = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 { buffer.push('.'); }
        buffer.push(ch);
    }
    buffer
}

impl App {
    /// Perfil integral quando só há uma célula ou coluna inteira selecionada;
    /// intervalos com múltiplas linhas analisam somente essas linhas.
    pub(super) fn abrir_perfil_coluna(&mut self) -> Task<Msg> {
        self.estatisticas = None;
        self.estatisticas_geracao = self.estatisticas_geracao.wrapping_add(1);
        self.confirmar();
        let area = self.area_selecionada();
        let Some(aba) = self.workbook.aba(self.folha) else {
            self.status = tr!("Não foi possível localizar a planilha").into();
            return Task::none();
        };
        let ultima = aba.sheet.used_rect().r1;
        let inteira = area.r0 == area.r1 || area.is_full_col();
        let r0 = if inteira { 0 } else { area.r0 };
        let r1 = if inteira { ultima } else { area.r1.min(ultima) };
        let linhas = if r0 > r1 { 0 } else { u64::from(r1 - r0) + 1 };

        // Apenas c0; o TypeScript também perfila a primeira coluna selecionada.
        let coluna = area.c0;
        // Se o intervalo selecionado estiver abaixo do último dado usado,
        // não inverter r0/r1: Rect::normalized() o transformaria num
        // intervalo NÃO vazio e o perfil contaria linhas indevidas.
        let alvo = Rect::new(r0, coluna, r1.max(r0), coluna);
        self.perfil_geracao = self.perfil_geracao.wrapping_add(1);
        let geracao = self.perfil_geracao;
        if linhas > 4096 {
            // Captura apenas a coluna pedida; ordenação e estatísticas rodam
            // fora do event loop, sem limite artificial de linhas.
            let valores: Vec<_> = if r0 <= r1 {
                (r0..=r1).map(|r| self.workbook.valor(self.folha, r, coluna)).collect()
            } else { Vec::new() };
            self.perfil_coluna = None;
            self.menu_aberto = false;
            self.contexto = None;
            self.status = trf!("Analisando {} linhas da coluna…", quantidade(linhas as usize));
            return Task::perform(async move {
                tokio::task::spawn_blocking(move || {
                    let dados = perfil::perfil_coluna(alvo.clone(), ultima, |r, _| {
                        valores.get((r - r0) as usize).cloned().unwrap_or(feathersheet_core::Value::Empty)
                    });
                    PerfilVisao { coluna, area: alvo, inteira, dados }
                }).await.map_err(|e| e.to_string())
            }, move |resultado| Msg::PerfilColunaPronto(geracao, resultado));
        }
        let dados = perfil::perfil_coluna(alvo.clone(), ultima, |r, c| {
            self.workbook.valor(self.folha, r, c)
        });
        self.perfil_coluna = Some(PerfilVisao {
            coluna, area: alvo, inteira, dados,
        });
        self.menu_aberto = false;
        self.contexto = None;
        self.status = trf!(
            "Perfil da coluna {} — {} linhas analisadas",
            feathersheet_core::celulas::refs::col_name(coluna), quantidade(linhas as usize),
        );
        Task::none()
    }

    /// Diálogo do perfil: grades rótulo/valor por seção; seção sem dado não
    /// aparece, e coluna sem valor nenhum vira um aviso só.
    fn view_perfil_coluna(&self, visao: &PerfilVisao) -> super::dialogo_interno::Dialogo<'_, Msg> {
        use super::dialogo_interno::dlg;
        let p = &visao.dados;
        let coluna = feathersheet_core::celulas::refs::col_name(visao.coluna);
        let descricao = if visao.inteira {
            tr!("Da primeira à última linha usada na aba").to_owned()
        } else {
            trf!("Linhas {} a {}", visao.area.r0 + 1, visao.area.r1 + 1)
        };
        let preenchidas = p.linhas.saturating_sub(p.vazias);
        let par = |r: &str, v: String| (r.to_owned(), v);

        let mut corpo = pelicano::column(Vec::<Element<_>>::new()).spacing(8);
        if preenchidas == 0 {
            let onde = if visao.inteira { tr!("na aba").to_owned() } else {
                trf!("nas linhas {} a {}", visao.area.r0 + 1, visao.area.r1 + 1)
            };
            corpo = corpo.push(dlg::aviso_vazio(trf!(
                "A coluna {coluna} não tem valores {onde}. Escolha uma coluna com dados para ver o perfil.")));
        } else {
            corpo = corpo.push(dlg::secao(tr!("Visão geral"))).push(dlg::grade_rotulo_valor(vec![
                par(tr!("Linhas examinadas"), quantidade(p.linhas)),
                par(tr!("Preenchidas"), quantidade(preenchidas)),
                par(tr!("Vazias"), quantidade(p.vazias)),
                par(tr!("Distintos"), quantidade(p.distintos)),
                par(tr!("Repetidos"), quantidade(p.repetidos)),
                par(tr!("Números"), quantidade(p.numeros)),
                par(tr!("Textos"), quantidade(p.textos)),
                par(tr!("Booleanos"), quantidade(p.logicos)),
                par(tr!("Erros"), quantidade(p.erros)),
            ], 2));
            if p.numeros > 0 {
                let mut pares = vec![
                    par(tr!("Mínimo"), numero(p.min)),
                    par(tr!("Máximo"), numero(p.max)),
                    par(tr!("Média"), numero(p.media)),
                ];
                if let Some(q) = &p.quartis {
                    pares.extend([
                        par(tr!("Mediana"), numero(q.mediana)),
                        par(tr!("1º quartil"), numero(q.q1)),
                        par(tr!("3º quartil"), numero(q.q3)),
                        par(tr!("Limite inferior"), numero(q.limite_baixo)),
                        par(tr!("Limite superior"), numero(q.limite_alto)),
                        par(tr!("Fora da curva"), quantidade(p.outliers)),
                    ]);
                }
                corpo = corpo.push(dlg::secao(tr!("Estatísticas numéricas"))).push(dlg::grade_rotulo_valor(pares, 2));
            }
            if !p.histograma.is_empty() {
                let max_qtd = p.histograma.iter().map(|f| f.qtd).max().unwrap_or(1).max(1) as f32;
                let mut barras = pelicano::column(Vec::<Element<_>>::new()).spacing(4);
                for faixa in &p.histograma {
                    let rotulo = if faixa.de == faixa.ate {
                        numero(faixa.de)
                    } else {
                        format!("{} – {}", numero(faixa.de), numero(faixa.ate))
                    };
                    let largura = (faixa.qtd as f32 / max_qtd * 220.0).max(2.0);
                    let barra = container(Space::new(Length::Fixed(largura), Length::Fixed(12.0)))
                        .style(|_| pelicano::Style {
                            background: Some(pelicano::Background::Color(COR_AZUL_ACENTO())),
                            border: pelicano::Border { radius: [3.0; 4], ..Default::default() },
                            ..Default::default()
                        });
                    barras = barras.push(row![
                        text(rotulo).size(11.5).color(COR_TEXTO_MUTED()).width(Length::Fixed(150.0)),
                        barra,
                        text(quantidade(faixa.qtd)).size(11.5),
                    ].spacing(8).align(pelicano::Align::Center));
                }
                corpo = corpo.push(dlg::secao(tr!("Distribuição dos números")))
                    .push(container(barras).padding([8, 12]).width(Length::FILL).style(dlg::estilo_bloco));
            }
            if !p.frequentes.is_empty() {
                let pares = p.frequentes.iter()
                    .map(|(v, q)| (v.clone(), trf!("{} vezes", quantidade(*q)))).collect();
                corpo = corpo.push(dlg::secao(tr!("Valores mais repetidos"))).push(dlg::grade_rotulo_valor(pares, 1));
            }
            if !p.exemplos_outliers.is_empty() {
                let mut lista = pelicano::column(Vec::<Element<_>>::new()).spacing(4);
                for &(linha, valor) in &p.exemplos_outliers {
                    lista = lista.push(
                        button(row![
                            text(cell_name(linha, visao.coluna)).size(12.5).width(Length::Fixed(70.0)),
                            text(numero(valor)).size(12.5).width(Length::FILL),
                            text(tr!("Ir para")).size(12).color(COR_TEXTO_MUTED()),
                            feather_ui::icones::icone("chevron-right", 14.0),
                        ].spacing(6).align(pelicano::Align::Center))
                        .width(Length::FILL).padding([5, 10])
                        .on_press(Msg::PerfilColunaIrPara(linha))
                        .style(estilo_botao_pequeno(false))
                    );
                }
                corpo = corpo.push(dlg::secao(tr!("Exemplos fora da curva")))
                    .push(container(lista).padding(4).width(Length::FILL).style(dlg::estilo_bloco));
            }
        }
        self.dialogo(trf!("Perfil da coluna {coluna}"), corpo)
            .icone("columns")
            .subtitulo(trf!("{descricao} · somente leitura"))
            .largura(560.0)
            .ao_fechar(Msg::PerfilColunaFechar)
    }

    /// Diálogo logo abaixo da faixa, sem cobri-la.
    pub(super) fn sobrepor_perfil_coluna<'a>(
        &'a self, base: Element<'a, Msg>
    ) -> Element<'a, Msg> {
        let Some(visao) = &self.perfil_coluna else { return base; };
        self.view_perfil_coluna(visao).sobre(base)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::celulas::values::Value;

    #[test]
    fn menu_typescript_de_estatisticas_dispara_modal_proprio_rust() {
        let app = App::default();
        let acao = menu_nativo::Acao {
            id: "columnStats".into(), args: Vec::new(),
        };
        assert!(matches!(
            app.evento_menu(&acao), Some(Msg::EstatisticasAbrir)
        ));
    }

    #[test]
    fn perfil_coluna_integral_testa_outlier_e_navegacao_sem_mutar_planilha() {
        let mut app = App::default();
        for i in 0..9 {
            app.workbook.escrever_celula(0, i, 0, (i + 1).to_string());
        }
        app.workbook.escrever_celula(0, 9, 0, "100".into());
        app.update(Msg::PerfilColunaAbrir);
        let perfil = app.perfil_coluna.as_ref().expect("perfil aberto");
        assert_eq!(perfil.dados.linhas, 10);
        assert_eq!(perfil.dados.outliers, 1);
        assert_eq!(perfil.dados.exemplos_outliers, vec![(9, 100.0)]);
        let _: Element<'_, Msg> = app.view_perfil_coluna(perfil).painel();
        assert!(!app.alterado);
        assert_eq!(app.workbook.valor(0, 9, 0), Value::Number(100.0));
        app.update(Msg::PerfilColunaIrPara(9));
        assert!(app.perfil_coluna.is_none());
        assert_eq!((app.linha, app.coluna), (9, 0));
        assert!(!app.alterado);
    }

    #[test]
    fn perfil_coluna_respeita_subintervalo_e_conta_vazios() {
        let mut app = App::default();
        for (r, s) in ["1", "2", "", "2", "3"].iter().enumerate() {
            if !s.is_empty() { app.workbook.escrever_celula(0, r as u32, 0, (*s).into()); }
        }
        app.update(Msg::IrPara("A2:A4".into()));
        app.update(Msg::PerfilColunaAbrir);
        let visao = app.perfil_coluna.as_ref().unwrap();
        assert_eq!(visao.dados.linhas, 3);
        assert_eq!(visao.dados.vazias, 1);
        assert_eq!(visao.dados.numeros, 2);
        assert_eq!(visao.dados.repetidos, 1);
        assert!(!visao.inteira);
        app.update(Msg::PerfilColunaFechar);
        assert!(app.perfil_coluna.is_none());
    }

    #[test]
    fn coluna_vazia_monta_so_o_aviso() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 3, 0, "1".into());
        app.update(Msg::IrPara("B1".into()));
        app.update(Msg::PerfilColunaAbrir);
        let visao = app.perfil_coluna.as_ref().unwrap();
        assert_eq!(visao.dados.linhas, visao.dados.vazias);
        let _: Element<'_, Msg> = app.view_perfil_coluna(visao).painel();
    }

    #[test]
    fn perfil_coluna_selecao_apos_ultima_linha_retorna_zero() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "10".into());
        app.update(Msg::IrPara("A20:A25".into()));
        app.update(Msg::PerfilColunaAbrir);
        let p = &app.perfil_coluna.as_ref().unwrap().dados;
        assert_eq!(p.linhas, 0);
        assert_eq!(p.vazias, 0);
        assert!(p.histograma.is_empty());
    }

    #[test]
    fn perfil_coluna_grande_e_agendado_sem_limite_e_conserva_todas_linhas() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 120_000, 0, "1".into());
        app.update(Msg::PerfilColunaAbrir);
        assert!(app.perfil_coluna.is_none());
        assert!(app.status.contains("Analisando 120.001"));
        let dados = perfil::perfil_coluna(Rect::new(0, 0, 120_000, 0), 120_000, |r, _|
            if r == 120_000 { feathersheet_core::Value::Number(1.0) } else { feathersheet_core::Value::Empty });
        let visao = PerfilVisao { coluna: 0, area: Rect::new(0, 0, 120_000, 0), inteira: true, dados };
        app.update(Msg::PerfilColunaPronto(app.perfil_geracao, Ok(visao)));
        let p = &app.perfil_coluna.as_ref().unwrap().dados;
        assert_eq!(p.linhas, 120_001);
        assert_eq!(p.vazias, 120_000);
        assert_eq!(p.numeros, 1);
        assert!(!app.alterado);
    }

    #[test]
    fn perfil_coluna_mostra_frequencia_e_quartis_por_motor_do_core() {
        let mut app = App::default();
        for (r, v) in ["1", "2", "2", "4", "5"].iter().enumerate() {
            app.workbook.escrever_celula(0, r as u32, 0, (*v).into());
        }
        app.update(Msg::PerfilColunaAbrir);
        let p = &app.perfil_coluna.as_ref().unwrap().dados;
        assert_eq!(p.distintos, 4);
        assert_eq!(p.repetidos, 1);
        assert_eq!(p.frequentes, vec![("2".to_owned(), 2)]);
        assert!(p.quartis.is_some());
        assert_eq!(p.histograma.iter().map(|f| f.qtd).sum::<usize>(), 5);
    }
}
