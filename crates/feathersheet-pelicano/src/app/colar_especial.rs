//! Janela "Colar especial…" (Ctrl+Alt+V), no modelo do Excel: o que colar,
//! a operação com o que já está no destino, "Ignorar em branco" e
//! "Transpor". A conta de cada célula é do motor
//! (`feathersheet_core::operacoes::colagem`); aqui só se decide o que vai
//! para onde, numa transação (um Ctrl+Z desfaz a colagem inteira).
use super::*;
use feathersheet_core::operacoes::colagem::{combinar, OperacaoColagem};
use pelicano::checkbox; use pelicano::column; use pelicano::radio;

/// O quadro "Colar" do Excel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QueColar {
    #[default]
    Tudo,
    Formulas,
    Valores,
    Formatos,
    ValoresEFormatos,
    Larguras,
}

#[derive(Debug, Clone)]
pub enum Evento {
    Abrir,
    Colar(QueColar),
    Operacao(OperacaoColagem),
    IgnorarBranco(bool),
    Transpor(bool),
    /// "Colar": lê a área de transferência e volta em `Executar`.
    Confirmar,
    Executar(String),
    Fechar,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Estado {
    pub colar: QueColar,
    pub operacao: OperacaoColagem,
    pub ignorar_branco: bool,
    pub transpor: bool,
}

/// Uma célula da origem: conteúdo cru, resultado congelado e estilo.
struct Origem<'a> {
    raw: &'a str,
    valor: &'a str,
    estilo: Option<&'a EstiloParcial>,
}

impl App {
    pub(super) fn evento_colar_especial(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Abrir => {
                self.colar_menu_aberto = false;
                // Mantém as escolhas da última colagem, como o Excel.
                self.colar_especial = Some(self.colar_especial_ultimo);
            }
            Evento::Colar(q) => if let Some(e) = &mut self.colar_especial { e.colar = q; },
            Evento::Operacao(o) => if let Some(e) = &mut self.colar_especial { e.operacao = o; },
            Evento::IgnorarBranco(v) => if let Some(e) = &mut self.colar_especial { e.ignorar_branco = v; },
            Evento::Transpor(v) => if let Some(e) = &mut self.colar_especial { e.transpor = v; },
            Evento::Confirmar => {
                return Task::read_clipboard(|v|
                    Msg::ColarEspecialJanela(Evento::Executar(v.unwrap_or_default())));
            }
            Evento::Executar(dados) => {
                let Some(estado) = self.colar_especial.take() else { return Task::none() };
                self.colar_especial_ultimo = estado;
                let dados = if dados.is_empty() {
                    self.area_copiada.as_ref().map(|a| a.texto.clone()).unwrap_or_default()
                } else { dados };
                self.colar_com_opcoes(dados, estado);
            }
            Evento::Fechar => self.colar_especial = None,
        }
        Task::none()
    }

    /// Faz a colagem com as escolhas da janela. Valida tudo antes de
    /// escrever: um valor recusado pela validação de dados cancela o bloco.
    pub(super) fn colar_com_opcoes(&mut self, dados: String, e: Estado) {
        if !self.confirmar() { return; }
        let copia = self.area_copiada.as_ref().filter(|c| c.texto == dados);
        let precisa_interna = matches!(e.colar,
            QueColar::Formatos | QueColar::ValoresEFormatos | QueColar::Larguras);
        if precisa_interna && copia.is_none() {
            self.status = tr!("Esta opção requer células copiadas do FeatherSheet").into();
            return;
        }
        if dados.is_empty() && copia.is_none() { return; }
        if e.colar == QueColar::Larguras {
            let larguras = copia.map(|c| c.larguras.clone()).unwrap_or_default();
            self.colar_larguras(&larguras);
            return;
        }
        let externas = copia.is_none().then(|| feathersheet_io::csv::dividir_csv(&dados, '\t'));
        // A origem, célula a célula, sem copiar texto (só referências).
        let origem: Vec<Vec<Origem<'_>>> = match copia {
            Some(c) => c.celulas.iter().zip(&c.valores).map(|(linha, valores)|
                linha.iter().zip(valores).map(|(s, v)| Origem {
                    raw: &s.raw, valor: v, estilo: s.style.as_ref(),
                }).collect()).collect(),
            None => externas.as_ref().expect("texto externo").iter().map(|linha|
                linha.iter().map(|t| Origem { raw: t, valor: t, estilo: None }).collect()).collect(),
        };
        let (alt_o, larg_o) = (origem.len(), origem.iter().map(Vec::len).max().unwrap_or(0));
        let (alt_b, larg_b) = if e.transpor { (larg_o, alt_o) } else { (alt_o, larg_o) };
        let plano = match PlanoColagem::calcular(&self.area_selecionada(), alt_b, larg_b) {
            Ok(p) => p,
            Err(m) => { self.status = m.into(); return; }
        };
        let vazia = Origem { raw: "", valor: "", estilo: None };
        let fonte = |dr: usize, dc: usize| -> &Origem<'_> {
            let (or, oc) = if e.transpor { (dc % alt_o, dr % larg_o) } else { (dr % alt_o, dc % larg_o) };
            origem.get(or).and_then(|l| l.get(oc)).unwrap_or(&vazia)
        };
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        // 1. Planeja cada célula (sem escrever) e valida.
        let mut novas: Vec<(u32, u32, Option<Snap>)> = Vec::new();
        for dr in 0..plano.altura {
            for dc in 0..plano.largura {
                let (r, c) = (plano.r0 + dr as u32, plano.c0 + dc as u32);
                let o = fonte(dr, dc);
                if e.ignorar_branco && o.raw.is_empty() { continue; }
                let atual = aba.sheet.snapshot(r, c);
                let raw_atual = atual.as_ref().map(|s| s.raw.as_str()).unwrap_or("");
                let estilo_atual = atual.as_ref().and_then(|s| s.style.clone());
                let colado = match e.colar {
                    QueColar::Tudo | QueColar::Formulas => o.raw,
                    _ => o.valor,
                };
                let raw = if e.colar == QueColar::Formatos {
                    raw_atual.to_owned()
                } else {
                    combinar(raw_atual, colado, e.operacao).unwrap_or_else(|| raw_atual.to_owned())
                };
                let estilo = match e.colar {
                    QueColar::Tudo | QueColar::Formatos | QueColar::ValoresEFormatos =>
                        o.estilo.cloned(),
                    _ => estilo_atual,
                };
                novas.push((r, c, (!raw.is_empty() || estilo.is_some())
                    .then_some(Snap { raw, style: estilo })));
            }
        }
        let mut aviso = None;
        if e.colar != QueColar::Formatos {
            let regras = match validacao_ui::carregar_regras(aba.sheet.va.as_deref()) {
                Ok(v) => v,
                Err(erro) => { self.status = erro; return; }
            };
            if !regras.is_empty() {
                for (r, c, s) in &novas {
                    let texto = s.as_ref().map(|s| s.raw.as_str()).unwrap_or("");
                    match validacao_ui::validar_celula(&self.workbook, self.folha, *r, *c, texto, &regras) {
                        Ok(Some(a)) if aviso.is_none() => aviso = Some(trf!("{}: {a}", cell_name(*r, *c))),
                        Ok(_) => {}
                        Err(erro) => {
                            self.status = trf!("Colagem cancelada em {}: {erro}", cell_name(*r, *c));
                            return;
                        }
                    }
                }
            }
        }
        // 2. Escreve tudo numa transação.
        let quantas = novas.len();
        self.workbook.editar(self.folha, |sheet| {
            sheet.abrir_transacao();
            for (r, c, s) in novas { sheet.apply(r, c, s, true); }
            sheet.fechar_transacao();
        });
        self.alterado = true;
        let area = plano.area();
        self.trocar_selecao(area.r0, area.c0);
        self.fim_selecao = (area.r1, area.c1);
        self.status = trf!("{quantas} célula(s) coladas com Colar especial{}",
            aviso.map(|a| trf!("; aviso de validação: {a}")).unwrap_or_default());
    }

    /// "Larguras das colunas": só as larguras da origem, a partir da coluna
    /// ativa, sem tocar no conteúdo.
    fn colar_larguras(&mut self, larguras: &[Option<u32>]) {
        let c0 = self.area_selecionada().c0;
        let mut mudou = false;
        self.workbook.editar(self.folha, |sheet| {
            sheet.abrir_transacao();
            for (i, w) in larguras.iter().enumerate() {
                let c = c0 + i as u32;
                if c >= MAX_COLS { break; }
                let largura = w.unwrap_or(crate::grade::LARGURA_PADRAO as u32);
                mudou |= sheet.definir_dimensao(true, c, largura);
            }
            sheet.fechar_transacao();
        });
        self.alterado |= mudou;
        self.status = trf!("Largura de {} coluna(s) colada", larguras.len());
    }

    /// O painel sozinho (testes).
    #[cfg(test)]
    fn view_colar_especial(&self) -> Element<'_, Msg> {
        self.dialogo_colar_especial().painel()
    }

    fn dialogo_colar_especial(&self) -> super::dialogo_interno::Dialogo<'_, Msg> {
        let e = self.colar_especial.as_ref().expect("janela aberta");
        let ev = Msg::ColarEspecialJanela;
        let grupo = |t: &'static str| text(t).size(10.5).color(COR_TEXTO_MUTED());
        let opcao = |rotulo: &'static str, q: QueColar|
            radio(rotulo, q, Some(e.colar), move |q| ev(Evento::Colar(q))).size(12.5);
        let conta = |rotulo: &'static str, o: OperacaoColagem|
            radio(rotulo, o, Some(e.operacao), move |o| ev(Evento::Operacao(o))).size(12.5);
        let duas = |a: Element<'static, Msg>, b: Element<'static, Msg>| row![
            container(a).width(Length::Fill(1)),
            container(b).width(Length::Fill(1)),
        ];
        let sem_conta = e.colar == QueColar::Formatos || e.colar == QueColar::Larguras;
        let mut operacoes = column![
            grupo(tr!("OPERAÇÃO")),
            duas(conta(tr!("Nenhuma"), OperacaoColagem::Nenhuma).into(),
                conta(tr!("Multiplicar"), OperacaoColagem::Multiplicar).into()),
            duas(conta(tr!("Somar"), OperacaoColagem::Somar).into(),
                conta(tr!("Dividir"), OperacaoColagem::Dividir).into()),
            duas(conta(tr!("Subtrair"), OperacaoColagem::Subtrair).into(), Space::new(0, 0.0).into()),
        ].spacing(8);
        if sem_conta {
            operacoes = operacoes.push(row![
                feather_ui::icones::icone("info-circle", 14.0),
                text(tr!("A operação não vale para formatos e larguras.")).size(11)
                    .color(COR_TEXTO_MUTED()),
            ].spacing(6).align(pelicano::Align::Center));
        }
        let corpo = column![
            column![
                grupo("COLAR"),
                duas(opcao(tr!("Tudo"), QueColar::Tudo).into(),
                    opcao(tr!("Valores e formatos"), QueColar::ValoresEFormatos).into()),
                duas(opcao(tr!("Fórmulas"), QueColar::Formulas).into(),
                    opcao(tr!("Larguras das colunas"), QueColar::Larguras).into()),
                duas(opcao(tr!("Valores"), QueColar::Valores).into(),
                    opcao(tr!("Formatos"), QueColar::Formatos).into()),
            ].spacing(8),
            operacoes,
            row![
                checkbox(tr!("Ignorar em branco"), e.ignorar_branco)
                    .on_toggle(move |v| ev(Evento::IgnorarBranco(v))).size(12.5),
                checkbox(tr!("Transpor"), e.transpor)
                    .on_toggle(move |v| ev(Evento::Transpor(v))).size(12.5),
            ].spacing(24),
        ].spacing(16);
        self.dialogo(tr!("Colar especial"), corpo)
            .icone("clipboard")
            .largura(480.0)
            .ao_fechar(ev(Evento::Fechar))
            .secundario(tr!("Cancelar"), Some(ev(Evento::Fechar)))
            .primario(tr!("Colar"), Some(ev(Evento::Confirmar)))
    }

    pub(super) fn sobrepor_colar_especial<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if self.colar_especial.is_none() { return base; }
        self.dialogo_colar_especial().sobre(base)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::Value;

    fn copiar(app: &mut App, area: &str) -> String {
        let _ = app.update(Msg::IrPara(area.into()));
        app.copiar_selecao().expect("copiou")
    }

    fn colar(app: &mut App, destino: &str, dados: String, e: Estado) {
        let _ = app.update(Msg::IrPara(destino.into()));
        app.colar_especial = Some(e);
        let _ = app.update(Msg::ColarEspecialJanela(Evento::Executar(dados)));
    }

    #[test]
    fn somar_valores_sobre_numeros_e_um_desfazer_volta() {
        let mut app = App::default();
        for (r, v) in ["10", "20"].iter().enumerate() { app.workbook.escrever_celula(0, r as u32, 0, (*v).into()); }
        for (r, v) in ["1", "2"].iter().enumerate() { app.workbook.escrever_celula(0, r as u32, 1, (*v).into()); }
        let dados = copiar(&mut app, "A1:A2");
        colar(&mut app, "B1", dados, Estado {
            colar: QueColar::Valores, operacao: OperacaoColagem::Somar, ..Default::default() });
        assert_eq!(app.workbook.valor(0, 0, 1), Value::Number(11.));
        assert_eq!(app.workbook.valor(0, 1, 1), Value::Number(22.));
        let _ = app.update(Msg::Desfazer);
        assert_eq!(app.workbook.valor(0, 0, 1), Value::Number(1.));
        assert_eq!(app.workbook.valor(0, 1, 1), Value::Number(2.));
    }

    #[test]
    fn ignorar_em_branco_preserva_o_destino_e_transpor_vira_linha() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "x".into());
        app.workbook.escrever_celula(0, 2, 0, "z".into());
        app.workbook.escrever_celula(0, 0, 3, "manter".into());
        app.workbook.escrever_celula(0, 0, 4, "manter".into());
        let dados = copiar(&mut app, "A1:A3");
        colar(&mut app, "C1", dados, Estado { ignorar_branco: true, transpor: true, ..Default::default() });
        let bruto = |c| app.workbook.aba(0).unwrap().sheet.get_raw(0, c);
        assert_eq!((bruto(2), bruto(3), bruto(4)), ("x".into(), "manter".into(), "z".into()));
    }

    #[test]
    fn larguras_copiam_so_a_largura_e_janela_monta_nos_dois_temas() {
        let mut app = App::default();
        app.workbook.editar(0, |s| { s.definir_dimensao(true, 0, 200); });
        app.workbook.escrever_celula(0, 0, 0, "a".into());
        let dados = copiar(&mut app, "A1:B1");
        colar(&mut app, "D1", dados, Estado { colar: QueColar::Larguras, ..Default::default() });
        let larguras = &app.workbook.aba(0).unwrap().sheet.col_widths;
        assert_eq!(larguras.get(&3).copied(), Some(200));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 3), "");
        let _ = app.update(Msg::ColarEspecialJanela(Evento::Abrir));
        for escuro in [false, true] {
            feather_ui::tema::definir_tema(escuro, 0);
            let _: Element<'_, Msg> = app.view_colar_especial();
        }
    }
}
