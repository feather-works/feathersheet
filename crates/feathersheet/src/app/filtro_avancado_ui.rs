//! Dados ▸ Classificar e filtrar ▸ Avançado, como o Filtro Avançado do Excel: a lista (primeira
//! linha = cabeçalhos), o intervalo de critérios (mesmos cabeçalhos; linhas = OU, colunas = E),
//! filtrar no lugar (oculta as linhas que não passam) ou copiar para outro lugar, e "Somente
//! registros exclusivos". A regra dos critérios é a mesma das macros
//! (`feathersheet_macros::filtro_avancado`). Tudo num Ctrl+Z.
use super::*;
use feathersheet_core::celulas::refs::{cell_name, parse_cell_ref, parse_range_ref, Rect};
use feathersheet_macros::filtro_avancado::linhas_que_passam;

#[derive(Debug, Clone)]
pub enum Evento {
    Abrir,
    Lista(String),
    Criterios(String),
    Copiar(bool),
    Destino(String),
    Unicos(bool),
    Aplicar,
    Fechar,
}

#[derive(Debug, Clone, Default)]
pub(super) struct EstadoFiltroAvancado {
    lista: String,
    criterios: String,
    copiar: bool,
    destino: String,
    unicos: bool,
    erro: Option<String>,
}

fn intervalo(t: &str) -> Option<Rect> {
    let t = t.trim().replace('$', "");
    parse_range_ref(&t).or_else(|| parse_range_ref(&format!("{t}:{t}"))).filter(|r| r.sheet.is_none()).map(|r| r.normalized())
}

impl App {
    pub(super) fn evento_filtro_avancado(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Abrir => {
                if !self.confirmar() { return Task::none(); }
                let a = self.area_selecionada();
                // Uma célula só: a região em volta, como o Excel propõe.
                let lista = match self.workbook.aba(self.folha) {
                    Some(aba) if a.r0 == a.r1 && a.c0 == a.c1 => feathersheet_core::operacoes::ops::regiao_atual(&aba.sheet, a),
                    _ => a,
                };
                self.filtro_avancado = Some(EstadoFiltroAvancado {
                    lista: format!("{}:{}", cell_name(lista.r0, lista.c0), cell_name(lista.r1, lista.c1)), ..Default::default() });
            }
            Evento::Lista(t) => if let Some(e) = &mut self.filtro_avancado { e.lista = t; },
            Evento::Criterios(t) => if let Some(e) = &mut self.filtro_avancado { e.criterios = t; },
            Evento::Copiar(v) => if let Some(e) = &mut self.filtro_avancado { e.copiar = v; },
            Evento::Destino(t) => if let Some(e) = &mut self.filtro_avancado { e.destino = t; },
            Evento::Unicos(v) => if let Some(e) = &mut self.filtro_avancado { e.unicos = v; },
            Evento::Aplicar => {
                let Some(e) = self.filtro_avancado.clone() else { return Task::none() };
                match self.aplicar_filtro_avancado(&e) {
                    Ok(msg) => { self.filtro_avancado = None; self.alterado = true; self.status = msg; }
                    Err(erro) => if let Some(s) = &mut self.filtro_avancado { s.erro = Some(erro); },
                }
            }
            Evento::Fechar => self.filtro_avancado = None,
        }
        Task::none()
    }

    fn aplicar_filtro_avancado(&mut self, e: &EstadoFiltroAvancado) -> Result<String, String> {
        let lista = intervalo(&e.lista).ok_or_else(|| tr!("Intervalo da lista inválido. Use, por exemplo, A1:D200.").to_string())?;
        if lista.r1 <= lista.r0 { return Err(tr!("A lista precisa dos cabeçalhos e de pelo menos uma linha de dados.").into()); }
        if lista.area() > 5_000_000 { return Err(tr!("Lista grande demais (mais de 5 milhões de células).").into()); }
        let criterios = if e.criterios.trim().is_empty() { None } else {
            Some(intervalo(&e.criterios).ok_or_else(|| tr!("Intervalo de critérios inválido. Use, por exemplo, F1:G2.").to_string())?)
        };
        let passam = linhas_que_passam(&self.workbook, self.folha, &lista, criterios.as_ref(), e.unicos)?;
        let aba = self.workbook.aba(self.folha).ok_or("Aba inexistente")?;
        let fim = lista.r1.min(aba.sheet.max_row);
        if !e.copiar {
            let mut erro = Ok(0);
            self.workbook.editar(self.folha, |s| {
                s.abrir_transacao();
                erro = s.alterar_visibilidade_dimensoes(false, false, lista.r0 + 1, fim);
                let mut r = lista.r0 + 1;
                for &p in passam.iter().chain(std::iter::once(&(fim + 1))) {
                    if erro.is_ok() && p > r { erro = s.alterar_visibilidade_dimensoes(false, true, r, p - 1); }
                    r = p + 1;
                }
                s.fechar_transacao();
            });
            erro?;
            return Ok(trf!("Filtro avançado: {} de {} linha(s) à mostra. Ctrl+Z desfaz.", passam.len(), fim - lista.r0));
        }
        let destino = e.destino.trim().replace('$', "");
        let (dr, dc) = parse_cell_ref(destino.split(':').next().unwrap_or("")).filter(|_| !destino.contains('!'))
            
            .ok_or_else(|| tr!("Destino inválido. Use uma célula desta aba, por exemplo, H1.").to_string())?;
        let (alt, larg) = (passam.len() as u32 + 1, lista.c1 - lista.c0 + 1);
        let alvo = Rect::new(dr, dc, dr + alt - 1, dc + larg - 1);
        if alvo.r1 >= MAX_ROWS || alvo.c1 >= MAX_COLS { return Err(tr!("O resultado não cabe a partir desse destino.").into()); }
        let cruza = |a: &Rect, b: &Rect| a.r0 <= b.r1 && b.r0 <= a.r1 && a.c0 <= b.c1 && b.c0 <= a.c1;
        if cruza(&alvo, &lista) || criterios.as_ref().is_some_and(|c| cruza(&alvo, c)) {
            return Err(tr!("O destino não pode ficar em cima da lista nem dos critérios.").into());
        }
        // Nada some calado: com dados no caminho, recusa (o Excel escreveria por cima).
        if (alvo.r0..=alvo.r1).any(|r| (alvo.c0..=alvo.c1).any(|c| !aba.sheet.get_raw(r, c).is_empty())) {
            return Err(trf!("Há dados em {}:{}. Escolha um destino vazio.", cell_name(alvo.r0, alvo.c0), cell_name(alvo.r1, alvo.c1)));
        }
        // shortcut: fórmula vai como o valor que mostra (texto como "001" vindo de fórmula vira número); copiar a fórmula se pedirem.
        let linhas: Vec<u32> = std::iter::once(lista.r0).chain(passam.iter().copied()).collect();
        let mut novas = Vec::new();
        for (i, &r) in linhas.iter().enumerate() {
            for (j, c) in (lista.c0..=lista.c1).enumerate() {
                let Some(snap) = aba.sheet.snapshot(r, c) else { continue };
                let raw = if snap.raw.starts_with('=') {
                    match self.workbook.valor(self.folha, r, c) {
                        feathersheet_core::Value::Number(n) => feathersheet_core::celulas::number::to_js_string(n),
                        feathersheet_core::Value::Empty => String::new(),
                        v => feathersheet_core::celulas::values::to_text(&v),
                    }
                } else { snap.raw };
                novas.push((dr + i as u32, dc + j as u32, Snap { raw, style: snap.style }));
            }
        }
        self.workbook.editar(self.folha, |s| {
            s.abrir_transacao();
            for (r, c, snap) in novas { s.apply(r, c, Some(snap), true); }
            s.fechar_transacao();
        });
        self.trocar_selecao(alvo.r0, alvo.c0);
        self.fim_selecao = (alvo.r1, alvo.c1);
        Ok(trf!("Filtro avançado: {} linha(s) copiadas para {}. Ctrl+Z desfaz.", passam.len(), cell_name(dr, dc)))
    }

    fn view_filtro_avancado<'a>(&'a self) -> super::dialogo_interno::Dialogo<'a, Msg> {
        let e = self.filtro_avancado.as_ref().expect("janela Filtro avançado aberta");
        let ev = Msg::FiltroAvancado;
        let rotulo = |t: &'static str| text(tr!(t)).size(12).color(COR_TEXTO_MUTED());
        let campo = |dica: &'static str, valor: &'a str, f: fn(String) -> Evento| text_input(dica, valor)
            .on_input(move |t| ev(f(t))).on_submit(ev(Evento::Aplicar)).size(12).padding([6, 8]).width(Length::FILL);
        let mut painel = pelicano::column![
            rotulo("Ação"),
            row![
                pelicano::radio(tr!("Filtrar a lista no local"), false, Some(e.copiar), move |v| ev(Evento::Copiar(v))).size(12.5),
                pelicano::radio(tr!("Copiar para outro local"), true, Some(e.copiar), move |v| ev(Evento::Copiar(v))).size(12.5),
            ].spacing(16),
            rotulo("Intervalo da lista"),
            campo("A1:D200", &e.lista, Evento::Lista),
            rotulo("Intervalo de critérios (opcional)"),
            campo("F1:G2", &e.criterios, Evento::Criterios),
        ].spacing(8);
        if e.copiar {
            painel = painel.push(rotulo("Copiar para")).push(campo("H1", &e.destino, Evento::Destino));
        }
        painel = painel.push(pelicano::checkbox(tr!("Somente registros exclusivos"), e.unicos).on_toggle(move |v| ev(Evento::Unicos(v))).size(12.5));
        if let Some(erro) = &e.erro {
            painel = painel.push(row![feather_ui::icones::icone("info-circle", 15.0), text(erro.clone()).size(12).color(COR_ERRO())]
                .spacing(6).align(pelicano::Align::Center));
        }
        let pode = !e.lista.trim().is_empty() && (!e.copiar || !e.destino.trim().is_empty());
        self.dialogo(tr!("Filtro avançado"), painel)
            .icone("filter")
            .subtitulo(tr!("Critérios: os mesmos cabeçalhos da lista; cada linha é uma alternativa (OU), cada coluna soma uma condição (E)."))
            .largura(480.0)
            .ao_fechar(ev(Evento::Fechar))
            .secundario(tr!("Cancelar"), Some(ev(Evento::Fechar)))
            .primario("OK", pode.then_some(ev(Evento::Aplicar)))
    }

    pub(super) fn sobrepor_filtro_avancado<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if self.filtro_avancado.is_none() { return base; }
        self.view_filtro_avancado().sobre(base)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn exemplo() -> App {
        let mut app = App::default();
        for (r, (uf, v)) in [("UF", "Valor"), ("SP", "10"), ("RJ", "200"), ("SP", "300"), ("SP", "300")].iter().enumerate() {
            app.workbook.escrever_celula(0, r as u32, 0, (*uf).into());
            app.workbook.escrever_celula(0, r as u32, 1, (*v).into());
        }
        app.workbook.escrever_celula(0, 0, 4, "UF".into());
        app.workbook.escrever_celula(0, 1, 4, "SP".into());
        app.workbook.escrever_celula(0, 0, 5, "Valor".into());
        app.workbook.escrever_celula(0, 1, 5, ">100".into());
        let _ = app.update(Msg::Selecionar(0, 0));
        let _ = app.update(Msg::FiltroAvancado(Evento::Abrir));
        let _ = app.update(Msg::FiltroAvancado(Evento::Criterios("E1:F2".into())));
        app
    }

    #[test]
    fn no_local_oculta_quem_nao_passa_e_um_desfazer_volta() {
        let mut app = exemplo();
        assert_eq!(app.filtro_avancado.as_ref().unwrap().lista, "A1:B5", "a região da célula ativa");
        let _ = app.view_filtro_avancado();
        let _ = app.update(Msg::FiltroAvancado(Evento::Aplicar));
        assert!(app.filtro_avancado.is_none(), "{}", app.status);
        let s = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!((1..=4).map(|r| s.linha_oculta(r)).collect::<Vec<_>>(), vec![true, true, false, false]);
        let _ = app.update(Msg::Desfazer);
        assert!(!(1..=4).any(|r| app.workbook.aba(0).unwrap().sheet.linha_oculta(r)));
    }

    #[test]
    fn copiar_unicos_para_destino_vazio_e_recusa_destino_com_dados() {
        let mut app = exemplo();
        let _ = app.update(Msg::FiltroAvancado(Evento::Copiar(true)));
        let _ = app.update(Msg::FiltroAvancado(Evento::Unicos(true)));
        let _ = app.update(Msg::FiltroAvancado(Evento::Destino("E1".into())));
        let _ = app.update(Msg::FiltroAvancado(Evento::Aplicar));
        assert!(app.filtro_avancado.as_ref().unwrap().erro.is_some(), "E1 cruza os critérios");
        let _ = app.update(Msg::FiltroAvancado(Evento::Destino("H1".into())));
        let _ = app.update(Msg::FiltroAvancado(Evento::Aplicar));
        assert!(app.filtro_avancado.is_none(), "{}", app.status);
        let s = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!((0..3).map(|r| s.get_raw(r, 8)).collect::<Vec<_>>(), vec!["Valor", "300", ""], "cabeçalho e uma linha só");
    }
}
