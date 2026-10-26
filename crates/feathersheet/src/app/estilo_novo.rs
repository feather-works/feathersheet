//! Estilos de célula da pessoa (Página Inicial → Estilos de célula ▾ →
//! Novo estilo de célula…), como no Excel: um nome e as partes do formato
//! da célula ativa que o estilo leva (Número, Alinhamento, Fonte, Borda,
//! Preenchimento). Ficam na pasta (`Workbook::estilos_celula`, gravado no
//! `.fshz`) e aparecem em "Personalizados" no mesmo menu. Aplicar troca só
//! as partes que o estilo leva, num Ctrl+Z.
use super::*;
use feathersheet_io::da_workbook::estilo_parcial_para_json;
use feathersheet_io::para_workbook::estilo_parcial_de_json;
use serde_json::{json, Value as Json};

/// As partes de um estilo e os campos do motor de cada uma.
pub(super) const GRUPOS: [(&str, &str, &[&str]); 5] = [
    ("numero", "Número", &["format", "decimals"]),
    ("alinhamento", "Alinhamento", &["align", "vAlign", "wrap", "rotation", "indent", "textVertical"]),
    ("fonte", "Fonte", &["fontFamily", "fontSize", "bold", "italic", "underline", "strike", "color"]),
    ("borda", "Borda", &["borders"]),
    ("preenchimento", "Preenchimento", &["bg"]),
];

#[derive(Debug, Clone)]
pub(super) struct EstiloPessoal {
    pub nome: String,
    pub grupos: Vec<String>,
    pub estilo: EstiloParcial,
}

#[derive(Debug, Clone)]
pub enum Evento { Abrir, Nome(String), Grupo(usize, bool), Salvar, Fechar, Aplicar(usize), Excluir(usize) }

#[derive(Debug, Clone)]
pub(super) struct Estado {
    pub nome: String,
    pub grupos: [bool; 5],
    /// O estilo da célula ativa no momento em que a janela abriu.
    pub base: EstiloParcial,
    pub erro: Option<String>,
}

/// Lê a lista guardada na pasta; entrada estranha é ignorada.
pub(super) fn da_pasta(w: &Workbook) -> Vec<EstiloPessoal> {
    let Some(lista) = w.estilos_celula.as_deref().and_then(|t| serde_json::from_str::<Vec<Json>>(t).ok()) else {
        return Vec::new();
    };
    lista.iter().filter_map(|e| Some(EstiloPessoal {
        nome: e["nome"].as_str()?.to_string(),
        grupos: e["grupos"].as_array()?.iter().filter_map(|g| g.as_str().map(String::from)).collect(),
        estilo: estilo_parcial_de_json(&e["estilo"]).unwrap_or_default(),
    })).collect()
}

fn gravar(w: &mut Workbook, lista: &[EstiloPessoal]) {
    w.estilos_celula = (!lista.is_empty()).then(|| Json::Array(lista.iter().map(|e| json!({
        "nome": e.nome, "grupos": e.grupos, "estilo": estilo_parcial_para_json(&e.estilo),
    })).collect()).to_string());
}

/// Uma linha curta dizendo o que a parte `grupo` da célula tem, como a
/// coluna da direita da janela Estilo do Excel.
fn resumo(grupo: &str, e: &EstiloParcial) -> String {
    let s = feathersheet_core::celulas::estilo_parcial::para_style(e);
    use feathersheet_core::celulas::style::{Align, NumberFormat, VAlign};
    match grupo {
        "numero" => {
            let nome = match s.format {
                NumberFormat::Auto => tr!("Geral"), NumberFormat::Number => tr!("Número"), NumberFormat::Integer => tr!("Inteiro"),
                NumberFormat::Currency => tr!("Moeda"), NumberFormat::Accounting => tr!("Contábil"), NumberFormat::Percent => tr!("Porcentagem"),
                NumberFormat::Scientific => tr!("Científico"), NumberFormat::Date => tr!("Data"), NumberFormat::Datetime => tr!("Data e hora"),
                NumberFormat::Time => tr!("Hora"), NumberFormat::Duration => tr!("Duração"), NumberFormat::Text => tr!("Texto"),
            };
            match s.decimals { Some(d) => trf!("{nome}, {d} casa(s)"), None => nome.into() }
        }
        "alinhamento" => {
            let h = match s.align { Align::Left => tr!("Esquerda"), Align::Center => tr!("Centro"), Align::Right => tr!("Direita"), Align::Auto => tr!("Geral") };
            let v = match s.valign { VAlign::Top => "topo", VAlign::Bottom => "base", VAlign::Middle => "meio" };
            trf!("{h}, {v}{}", if s.wrap { tr!(", quebra") } else { "" })
        }
        "fonte" => {
            let mut t = format!("{} {}", s.font_family.as_deref().and_then(|f| f.split(',').next()).map(|f| f.trim_matches(['\'', ' '])).unwrap_or(tr!("Padrão")),
                s.font_size.map(|n| n.to_string()).unwrap_or_else(|| "13".into()));
            for (sim, nome) in [(s.bold, "negrito"), (s.italic, tr!("itálico")), (s.underline, "sublinhado"), (s.strike, "tachado")] {
                if sim { t.push_str(", "); t.push_str(nome); }
            }
            if let Some(c) = &s.color { t.push_str(&trf!(", {c}")); }
            t
        }
        "borda" => match &s.borders {
            Some(b) if b.top && b.right && b.bottom && b.left => tr!("Todas").into(),
            Some(b) => {
                let lados: Vec<&str> = [(b.top, "superior"), (b.right, "direita"), (b.bottom, "inferior"), (b.left, "esquerda")]
                    .into_iter().filter(|x| x.0).map(|x| x.1).collect();
                if lados.is_empty() { tr!("Sem bordas").into() } else { lados.join(", ") }
            }
            None => tr!("Sem bordas").into(),
        },
        _ => s.bg.clone().unwrap_or_else(|| tr!("Sem preenchimento").into()),
    }
}

impl App {
    fn abrir_novo_estilo(&mut self) {
        if !self.confirmar() { return; }
        let base = self.workbook.aba(self.folha)
            .and_then(|a| a.sheet.estilo_mesclado(self.linha, self.coluna)).unwrap_or_default();
        let existentes = da_pasta(&self.workbook);
        let nome = (1..).map(|n| trf!("Estilo {n}")).find(|n| !existentes.iter().any(|e| e.nome.eq_ignore_ascii_case(n)))
            .unwrap_or_default();
        self.estilos_menu_aberto = false;
        self.novo_estilo = Some(Estado { nome, grupos: [true; 5], base, erro: None });
    }

    fn salvar_novo_estilo(&mut self) {
        let Some(s) = self.novo_estilo.clone() else { return };
        let nome = s.nome.trim().to_string();
        let erro = if nome.is_empty() {
            Some(tr!("Dê um nome ao estilo.").to_string())
        } else if !s.grupos.iter().any(|&g| g) {
            Some(tr!("Marque ao menos uma parte do formato.").into())
        } else if da_pasta(&self.workbook).iter().any(|e| e.nome.eq_ignore_ascii_case(&nome)) {
            Some(trf!("Já existe um estilo chamado \"{nome}\"."))
        } else { None };
        if let Some(erro) = erro {
            if let Some(m) = &mut self.novo_estilo { m.erro = Some(erro); }
            return;
        }
        let mut grupos = Vec::new();
        let mut estilo = EstiloParcial::new();
        for (i, (id, _, campos)) in GRUPOS.iter().enumerate() {
            if !s.grupos[i] { continue; }
            grupos.push(id.to_string());
            for &k in *campos {
                if let Some(v) = s.base.get(k) { estilo.insert(k.into(), v.clone()); }
            }
        }
        let mut lista = da_pasta(&self.workbook);
        lista.push(EstiloPessoal { nome: nome.clone(), grupos, estilo });
        gravar(&mut self.workbook, &lista);
        self.alterado = true;
        self.novo_estilo = None;
        self.status = trf!("Estilo \"{nome}\" criado: está em Estilos de célula → Personalizados.");
    }

    /// Aplica o estilo `i` na seleção: só as partes que ele leva mudam (as
    /// outras, como a borda de um estilo só de fonte, ficam).
    pub(super) fn aplicar_estilo_pessoal(&mut self, i: usize) {
        let Some(e) = da_pasta(&self.workbook).into_iter().nth(i) else { return };
        self.confirmar();
        let mut patch = EstiloParcial::new();
        for (id, _, campos) in GRUPOS {
            if e.grupos.iter().any(|g| g == id) {
                for &k in campos { patch.insert(k.into(), Campo::Nulo); }
            }
        }
        for (k, v) in &e.estilo { patch.insert(k.clone(), v.clone()); }
        let areas = self.areas_selecionadas();
        self.workbook.editar(self.folha, |sheet| {
            sheet.abrir_transacao();
            for a in &areas { sheet.set_style(a, &patch); }
            sheet.fechar_transacao();
        });
        self.alterado = true;
        self.estilos_menu_aberto = false;
        self.status = trf!("Estilo de célula \"{}\" aplicado.", e.nome);
    }

    pub(super) fn evento_novo_estilo(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Abrir => self.abrir_novo_estilo(),
            Evento::Nome(n) => if let Some(m) = &mut self.novo_estilo { m.nome = n; m.erro = None; },
            Evento::Grupo(i, v) => if let Some(m) = &mut self.novo_estilo { m.grupos[i] = v; m.erro = None; },
            Evento::Salvar => self.salvar_novo_estilo(),
            Evento::Fechar => self.novo_estilo = None,
            Evento::Aplicar(i) => self.aplicar_estilo_pessoal(i),
            Evento::Excluir(i) => {
                let mut lista = da_pasta(&self.workbook);
                if i < lista.len() {
                    let nome = lista.remove(i).nome;
                    gravar(&mut self.workbook, &lista);
                    self.alterado = true;
                    self.status = trf!("Estilo \"{nome}\" excluído (as células que o usavam mantêm o formato).");
                }
            }
        }
        Task::none()
    }

    fn view_novo_estilo(&self) -> super::dialogo_interno::Dialogo<'_, Msg> {
        let s = self.novo_estilo.as_ref().expect("janela Novo estilo aberta");
        let campo = text_input(tr!("Nome do estilo"), &s.nome)
            .on_input(|v| Msg::NovoEstilo(Evento::Nome(v)))
            .on_submit(Msg::NovoEstilo(Evento::Salvar))
            .padding(9).width(Length::FILL);
        let mut partes = pelicano::column(Vec::<Element<_>>::new()).spacing(8);
        for (i, (id, rotulo, _)) in GRUPOS.iter().enumerate() {
            partes = partes.push(row![
                container(pelicano::checkbox(tr!(*rotulo), s.grupos[i]).on_toggle(move |v| Msg::NovoEstilo(Evento::Grupo(i, v)))
                    .size(13)).width(Length::Fixed(140.0)),
                text(resumo(id, &s.base)).size(12).color(COR_TEXTO_MUTED()),
            ].align(pelicano::Align::Center));
        }
        let celula = trf!("O estilo inclui (da célula {}):", feathersheet_core::celulas::refs::cell_name(self.linha, self.coluna));
        let mut painel = pelicano::column![
            row![text(tr!("Nome do estilo")).size(13), campo].spacing(12).align(pelicano::Align::Center),
            text(celula).size(12).color(COR_TEXTO_MUTED()),
            partes,
        ].spacing(12);
        if let Some(erro) = &s.erro {
            painel = painel.push(row![
                feather_ui::icones::icone("info-circle", 15.0),
                text(erro.clone()).size(12).color(COR_ERRO()),
            ].spacing(6).align(pelicano::Align::Center));
        }
        self.dialogo(tr!("Novo estilo de célula"), painel)
            .icone("palette")
            .largura(480.0)
            .ao_fechar(Msg::NovoEstilo(Evento::Fechar))
            .secundario(tr!("Cancelar"), Some(Msg::NovoEstilo(Evento::Fechar)))
            .primario("OK", Some(Msg::NovoEstilo(Evento::Salvar)))
    }

    pub(super) fn sobrepor_novo_estilo<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if self.novo_estilo.is_none() { return base; }
        self.view_novo_estilo().sobre(base)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn estilo(app: &App, r: u32, c: u32) -> feathersheet_core::celulas::style::Style {
        app.workbook.aba(0).unwrap().sheet.estilo_resolvido(r, c).unwrap_or_default()
    }

    #[test]
    fn cria_da_celula_ativa_aplica_so_as_partes_marcadas_e_desfaz() {
        let mut app = App::default();
        let _ = app.update(Msg::Selecionar(0, 0));
        let _ = app.update(Msg::EstiloMenu("bold".into(), Campo::Logico(true)));
        let _ = app.update(Msg::EstiloMenu("bg".into(), Campo::Texto("#fff2b2".into())));
        let _ = app.update(Msg::NovoEstilo(Evento::Abrir));
        let m = app.novo_estilo.as_ref().expect("abriu");
        assert_eq!(m.nome, "Estilo 1");
        assert!(resumo("fonte", &m.base).contains("negrito") && resumo("preenchimento", &m.base) == "#fff2b2");
        let _: Element<'_, Msg> = app.view_novo_estilo().painel();
        let _ = app.update(Msg::NovoEstilo(Evento::Nome("Destaque".into())));
        let _ = app.update(Msg::NovoEstilo(Evento::Grupo(4, false))); // sem preenchimento
        let _ = app.update(Msg::NovoEstilo(Evento::Salvar));
        assert!(app.novo_estilo.is_none());
        assert_eq!(da_pasta(&app.workbook).len(), 1);
        // Em outra célula, com fundo próprio: o estilo põe negrito e não mexe no fundo.
        let _ = app.update(Msg::Selecionar(2, 2));
        let _ = app.update(Msg::EstiloMenu("bg".into(), Campo::Texto("#cdf4d9".into())));
        let _ = app.update(Msg::NovoEstilo(Evento::Aplicar(0)));
        let s = estilo(&app, 2, 2);
        assert!(s.bold && s.bg.as_deref() == Some("#cdf4d9"));
        let _ = app.update(Msg::Desfazer);
        assert!(!estilo(&app, 2, 2).bold, "um Ctrl+Z desfaz o estilo");
    }

    #[test]
    fn recusa_nome_repetido_e_nenhuma_parte_e_exclui() {
        let mut app = App::default();
        let _ = app.update(Msg::NovoEstilo(Evento::Abrir));
        let _ = app.update(Msg::NovoEstilo(Evento::Salvar));
        let _ = app.update(Msg::NovoEstilo(Evento::Abrir));
        assert_eq!(app.novo_estilo.as_ref().unwrap().nome, "Estilo 2", "sugere o próximo nome livre");
        let _ = app.update(Msg::NovoEstilo(Evento::Nome("estilo 1".into())));
        let _ = app.update(Msg::NovoEstilo(Evento::Salvar));
        assert!(app.novo_estilo.as_ref().unwrap().erro.as_ref().unwrap().contains("Já existe"));
        for i in 0..5 { let _ = app.update(Msg::NovoEstilo(Evento::Grupo(i, false))); }
        let _ = app.update(Msg::NovoEstilo(Evento::Nome("Outro".into())));
        let _ = app.update(Msg::NovoEstilo(Evento::Salvar));
        assert!(app.novo_estilo.as_ref().unwrap().erro.as_ref().unwrap().contains("ao menos uma"));
        let _ = app.update(Msg::NovoEstilo(Evento::Fechar));
        let _ = app.update(Msg::NovoEstilo(Evento::Excluir(0)));
        assert!(app.workbook.estilos_celula.is_none());
    }
}
