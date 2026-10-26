//! Janela nativa "Formatar Células", com seis abas e rascunho separado da aba.
//! Somente OK modifica dados, em uma transação. O painel não duplica o motor.
use super::*;
mod visual;
mod cores_bordas;
mod orientacao_visual;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aba { Numero, Alinhamento, Fonte, Borda, Preenchimento, Protecao }
impl Aba {
    const TODAS: [(Self, &str); 6] = [
        (Self::Numero, "Número"),
        (Self::Alinhamento, "Alinhamento"),
        (Self::Fonte, "Fonte"),
        (Self::Borda, "Borda"),
        (Self::Preenchimento, "Preenchimento"),
        (Self::Protecao, "Proteção"),
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextoCampo { Fonte, Tamanho, CorTexto, CorFundo, CorBorda, Customizado, Decimais, Recuo, Angulo }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlvoCorDialogo { Fundo, Borda, Fonte }
impl AlvoCorDialogo {
    fn campo(self) -> TextoCampo {
        match self {
            Self::Fundo => TextoCampo::CorFundo,
            Self::Borda => TextoCampo::CorBorda,
            Self::Fonte => TextoCampo::CorTexto,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Evento {
    Aba(Aba),
    Campo(&'static str, Campo),
    Texto(TextoCampo, String),
    Formato(&'static str),
    FonteEstilo(u8),
    Efeito(&'static str),
    Borda(&'static str),
    CorPredefinida(&'static str),
    CorAlvo(AlvoCorDialogo),
    CorSeletor(seletor_cor::Evento),
    Grau(i16),
    Mesclar(bool),
    /// Abre ou fecha o seletor completo (as abas mostram só a paleta).
    MaisCores,
    Confirmar,
    Cancelar,
}

pub(super) struct Estado {
    pub aba: Aba,
    pub folha: usize,
    pub area: Rect,
    original: EstiloParcial,
    patch: EstiloParcial,
    fonte: String,
    tamanho: String,
    cor_texto: String,
    cor_fundo: String,
    cor_borda: String,
    personalizado: String,
    decimais: String,
    recuo: String,
    angulo: String,
    mesclar: bool,
    cor_alvo: AlvoCorDialogo,
    cor_hsva: [f32;4],
    cor_hex_picker: String,
    mais_cores: bool,
    erro: String,
}
impl Estado {
    fn novo(app: &App, aba: Aba) -> Self {
        let area = app.area_selecionada();
        let folha = app.folha;
        let original = app.workbook.aba(folha)
            .and_then(|a| a.sheet.estilo_mesclado(area.r0, area.c0))
            .unwrap_or_default();
        let string = |chave: &str| match original.get(chave) {
            Some(Campo::Texto(v)) => v.clone(),
            _ => String::new(),
        };
        let numero = |chave: &str, padrao: &str| match original.get(chave) {
            Some(Campo::Numero(v)) => format!("{v}"),
            _ => padrao.into(),
        };
        let mesclar = app.workbook.aba(folha).is_some_and(|a|
            mesclas::exatamente_mesclada(&a.sheet, &area));
        let cor_borda = match original.get("borders") {
            Some(Campo::Objeto(b)) => match b.get("color") {
                Some(Campo::Texto(v)) => v.clone(),
                _ => "#808080".into(),
            },
            _ => "#808080".into(),
        };
        let fonte = string("fontFamily");
        let cor_texto = string("color");
        let cor_fundo = string("bg");
        let personalizado = string("numFmt");
        let tamanho = numero("fontSize", "13");
        let decimais = numero("decimals", "2");
        let recuo = numero("indent", "0");
        let angulo = numero("rotation", "0");
        let cor_alvo = match aba {
            Aba::Preenchimento => AlvoCorDialogo::Fundo,
            Aba::Borda => AlvoCorDialogo::Borda,
            Aba::Fonte => AlvoCorDialogo::Fonte,
            _ => AlvoCorDialogo::Fundo,
        };
        let escolhida = match cor_alvo {
            AlvoCorDialogo::Fundo => &cor_fundo,
            AlvoCorDialogo::Borda => &cor_borda,
            AlvoCorDialogo::Fonte => &cor_texto,
        };
        let cor_hsva = seletor_cor::rgba_para_hsva(seletor_cor::rgba_de(escolhida),0.);
        let cor_hex_picker = if escolhida.is_empty() { "#ffffff".into() } else { escolhida.clone() };
        Self {
            aba, folha, area, original, patch: EstiloParcial::new(),
            fonte, tamanho, cor_texto, cor_fundo, cor_borda, personalizado,
            decimais, recuo, angulo, mesclar, cor_alvo, cor_hsva, cor_hex_picker,
            mais_cores: false, erro: String::new(),
        }
    }
    fn campo(&self, chave: &str) -> Option<&Campo> {
        self.patch.get(chave).or_else(|| self.original.get(chave))
    }
    fn booleano(&self, chave: &str) -> bool {
        matches!(self.campo(chave), Some(Campo::Logico(true)))
    }
    fn texto(&self, chave: &str) -> &str {
        match self.campo(chave) {
            Some(Campo::Texto(s)) => s,
            _ => "",
        }
    }
    fn bordas(&self) -> BTreeMap<String, Campo> {
        match self.campo("borders") {
            Some(Campo::Objeto(b)) => b.clone(),
            _ => BTreeMap::new(),
        }
    }
    fn cor_atual(&self) -> &str {
        match self.cor_alvo {
            AlvoCorDialogo::Fundo => &self.cor_fundo,
            AlvoCorDialogo::Borda => &self.cor_borda,
            AlvoCorDialogo::Fonte => &self.cor_texto,
        }
    }
    fn selecionar_alvo(&mut self, alvo: AlvoCorDialogo) {
        self.cor_alvo = alvo;
        let cor = self.cor_atual().to_owned();
        self.cor_hsva = seletor_cor::rgba_para_hsva(
            seletor_cor::rgba_de(&cor), self.cor_hsva[0]
        );
        self.cor_hex_picker = if cor.is_empty() { "#ffffff".into() } else { cor };
    }
    fn cor_do_hsva(&self) -> String {
        let [h,s,v,a]=self.cor_hsva;
        let [r,g,b]=seletor_cor::hsv_para_rgb(h,s,v);
        seletor_cor::hex_de([r,g,b,(a*255.).round() as u8])
    }
    fn cor_escolhida(&mut self, cor: String) {
        if !cor.is_empty() && !seletor_cor::hex_valido(&cor) {return;}
        if !cor.is_empty() {
            self.cor_hsva=seletor_cor::rgba_para_hsva(seletor_cor::rgba_de(&cor),self.cor_hsva[0]);
        }
        self.cor_hex_picker=cor.clone();
        self.mudar_texto(self.cor_alvo.campo(),cor);
    }
    fn seletor_evento(&mut self,evento:seletor_cor::Evento) {
        use seletor_cor::Evento as E;
        match evento {
            E::Fechar => {} // Incorporado à janela, não um popup descartável.
            E::Escolhida(cor) => self.cor_escolhida(cor),
            E::SaturacaoBrilho(s,v) => {
                self.cor_hsva[1]=s.clamp(0.,1.);
                self.cor_hsva[2]=v.clamp(0.,1.);
                self.cor_escolhida(self.cor_do_hsva());
            }
            E::Matiz(h) => {
                self.cor_hsva[0]=h.rem_euclid(360.);
                self.cor_escolhida(self.cor_do_hsva());
            }
            E::Alfa(a) => {
                self.cor_hsva[3]=a.clamp(0.,1.);
                self.cor_escolhida(self.cor_do_hsva());
            }
            E::TextoHex(valor) => {
                self.cor_hex_picker=valor.clone();
                if seletor_cor::hex_valido(&valor) {
                    self.cor_escolhida(format!("#{}",valor.trim().trim_start_matches('#').to_lowercase()));
                }
            }
            E::Canal(i,valor) => {
                if i>3 {return;}
                let Ok(n)=valor.trim().parse::<u16>() else {return;};
                if n>255 {return;}
                let mut rgba=seletor_cor::rgba_de(&self.cor_do_hsva());
                rgba[i]=n as u8;
                self.cor_escolhida(seletor_cor::hex_de(rgba));
            }
        }
    }
    fn mudar_texto(&mut self, campo: TextoCampo, valor: String) {
        self.erro.clear();
        let (chave, valor_atual): (&str, &mut String) = match campo {
            TextoCampo::Fonte => ("fontFamily", &mut self.fonte),
            TextoCampo::Tamanho => ("fontSize", &mut self.tamanho),
            TextoCampo::CorTexto => ("color", &mut self.cor_texto),
            TextoCampo::CorFundo => ("bg", &mut self.cor_fundo),
            TextoCampo::CorBorda => ("borderColor", &mut self.cor_borda),
            TextoCampo::Customizado => ("numFmt", &mut self.personalizado),
            TextoCampo::Decimais => ("decimals", &mut self.decimais),
            TextoCampo::Recuo => ("indent", &mut self.recuo),
            TextoCampo::Angulo => ("rotation", &mut self.angulo),
        };
        *valor_atual = valor.clone();
        let novo = match campo {
            TextoCampo::Tamanho => valor.parse::<f64>().ok()
                .filter(|v| v.is_finite() && (6.0..=96.0).contains(v))
                .map(Campo::Numero),
            TextoCampo::Decimais => valor.parse::<f64>().ok()
                .filter(|v| v.is_finite() && (0.0..=15.0).contains(v) && v.fract()==0.)
                .map(Campo::Numero),
            TextoCampo::Recuo => valor.parse::<f64>().ok()
                .filter(|v| v.is_finite() && (0.0..=50.0).contains(v) && v.fract()==0.)
                .map(Campo::Numero),
            TextoCampo::Angulo => valor.parse::<i16>().ok()
                .filter(|v| (-90..=90).contains(v)).map(|n| Campo::Numero(n as f64)),
            TextoCampo::CorTexto | TextoCampo::CorFundo | TextoCampo::CorBorda =>
                (valor.is_empty() || seletor_cor::hex_valido(&valor)).then(|| {
                    if valor.is_empty() { Campo::Nulo } else { Campo::Texto(valor.clone()) }
                }),
            _ => Some(if valor.trim().is_empty() { Campo::Nulo }
                else { Campo::Texto(valor.clone()) }),
        };
        if let Some(novo) = novo {
            if chave == "borderColor" {
                let mut b=self.bordas();
                if matches!(&novo,Campo::Nulo) {b.remove("color");}
                else {b.insert("color".into(),novo);}
                self.patch.insert("borders".into(),Campo::Objeto(b));
            } else {self.patch.insert(chave.into(),novo);}
            if matches!(campo,TextoCampo::CorTexto|TextoCampo::CorFundo|TextoCampo::CorBorda)
                && self.cor_alvo.campo()==campo {
                if !valor.is_empty() {
                    self.cor_hsva=seletor_cor::rgba_para_hsva(
                        seletor_cor::rgba_de(&valor),self.cor_hsva[0]);
                }
                self.cor_hex_picker=valor;
            }
        } else {
            self.erro = trf!("Valor inválido em {chave}. Corrija antes de confirmar.");
        }
    }
}

const FORMATOS: &[(&str, &str)] = &[
    ("Geral", "auto"), ("Número", "number"), ("Inteiro", "integer"),
    ("Moeda", "currency"), ("Contábil", "accounting"), ("Data", "date"),
    ("Data e hora", "datetime"), ("Hora", "time"),
    ("Percentual", "percent"), ("Científico", "scientific"),
    ("Duração", "duration"), ("Texto", "text"),
];
const CORES: &[&str] = &[
    "#ffffff","#f3f3f3","#dadce0","#9aa0a6","#5f6368","#202124",
    "#f4cccc","#fce5cd","#fff2cc","#d9ead3","#cfe2f3","#d9d2e9",
    "#ea9999","#f9cb9c","#ffe599","#b6d7a8","#a4c2f4","#b4a7d6",
    "#d93025","#e8710a","#fbbc04","#188038","#0b57d0","#9334e6",
];

impl App {
    pub(super) fn abrir_formatar_celulas(&mut self, aba: Aba) -> Task<Msg> {
        self.confirmar();
        self.orientacao_menu_aberto = false;
        if self.janelas.aberta(Auxiliar::FormatarCelulas) {
            if let Some(estado) = self.formato_celula.as_mut() { estado.aba = aba; }
            return Task::none();
        }
        self.formato_celula = Some(Estado::novo(self, aba));
        self.alternar(Auxiliar::FormatarCelulas)
    }

    pub(super) fn evento_formatar_celulas(&mut self, evento: Evento) -> Task<Msg> {
        if matches!(evento, Evento::Cancelar) {
            self.formato_celula = None;
            return self.janelas.fechar_tipo(Auxiliar::FormatarCelulas);
        }
        if matches!(evento, Evento::Confirmar) {
            let Some(estado) = self.formato_celula.as_ref() else { return Task::none(); };
            if !estado.erro.is_empty() { return Task::none(); }
            let estado = self.formato_celula.take().unwrap();
            let area = estado.area;
            let folha = estado.folha;
            let patch = estado.patch;
            let mesclar_alvo = estado.mesclar;
            let mut mudou = false;
            self.workbook.editar(folha, |sheet| {
                let mesclada = mesclas::exatamente_mesclada(sheet, &area);
                if patch.is_empty() && mesclar_alvo == mesclada { return; }
                // Uma única transação para todas as abas do painel.
                sheet.abrir_transacao();
                if !patch.is_empty() { sheet.set_style(&area, &patch); }
                if mesclar_alvo && !mesclada && area.area()>1 {
                    mesclas::mesclar(sheet, area.clone());
                } else if !mesclar_alvo && mesclada {
                    mesclas::desmesclar(sheet, area.clone());
                }
                sheet.fechar_transacao();
                mudou = true;
            });
            self.alterado |= mudou;
            if mudou { self.status = tr!("Formatação das células aplicada").into(); }
            return self.janelas.fechar_tipo(Auxiliar::FormatarCelulas);
        }
        let Some(e) = self.formato_celula.as_mut() else { return Task::none(); };
        match evento {
            Evento::Aba(aba) => {
                e.aba=aba; e.mais_cores=false;
                if let Some(alvo)=match aba {
                    Aba::Preenchimento=>Some(AlvoCorDialogo::Fundo),
                    Aba::Borda=>Some(AlvoCorDialogo::Borda),
                    Aba::Fonte=>Some(AlvoCorDialogo::Fonte),
                    _=>None,
                } {e.selecionar_alvo(alvo);}
            },
            Evento::Campo(chave, valor) => {
                e.patch.insert(chave.into(), valor);
                e.erro.clear();
            }
            Evento::Texto(campo, valor) => e.mudar_texto(campo, valor),
            Evento::Formato(nome) => {
                e.patch.insert("format".into(), if nome=="auto" {
                    Campo::Nulo
                } else { Campo::Texto(nome.into()) });
                // Formato da lista substitui o código personalizado.
                e.patch.insert("numFmt".into(), Campo::Nulo);
                e.personalizado.clear();
            }
            Evento::FonteEstilo(i) => {
                e.patch.insert("bold".into(), Campo::Logico(i==1 || i==3));
                e.patch.insert("italic".into(), Campo::Logico(i==2 || i==3));
            }
            Evento::Efeito(chave @ ("underline" | "doubleUnderline")) => {
                // Simples e duplo se excluem; o duplo grava os dois campos
                // (quem só conhece o simples ainda vê um traço).
                let duplo = e.booleano("doubleUnderline");
                let simples = e.booleano("underline") && !duplo;
                let ligar = if chave == "underline" { !simples } else { !duplo };
                e.patch.insert("underline".into(), Campo::Logico(ligar));
                e.patch.insert("doubleUnderline".into(),
                    if ligar && chave == "doubleUnderline" { Campo::Logico(true) } else { Campo::Nulo });
            }
            Evento::Efeito(chave) => {
                let ativo = e.booleano(chave);
                e.patch.insert(chave.into(), Campo::Logico(!ativo));
                if chave == "textVertical" && !ativo {
                    e.patch.insert("rotation".into(), Campo::Nulo);
                    e.angulo = "0".into();
                }
                if !ativo && chave=="superscript" {
                    e.patch.insert("subscript".into(), Campo::Logico(false));
                } else if !ativo && chave=="subscript" {
                    e.patch.insert("superscript".into(), Campo::Logico(false));
                }
            }
            Evento::Borda(opcao) => {
                // O botão pode mandar o rótulo traduzido; a decisão usa o português.
                let opcao = crate::idioma::original(opcao);
                let mut b = e.bordas();
                match opcao {
                    "Todas" | "Externas" | "Nenhuma" => {
                        let ligar=opcao!="Nenhuma";
                        for lado in ["top","right","bottom","left"] {
                            b.insert(lado.into(), Campo::Logico(ligar));
                        }
                    }
                    "Horizontais internas" => {
                        b.insert("top".into(), Campo::Logico(true));
                        b.insert("bottom".into(), Campo::Logico(true));
                    }
                    "Verticais internas" => {
                        b.insert("left".into(), Campo::Logico(true));
                        b.insert("right".into(), Campo::Logico(true));
                    }
                    lado @ ("top"|"bottom"|"left"|"right") => {
                        let ativo=matches!(b.get(lado),Some(Campo::Logico(true)));
                        b.insert(lado.into(),Campo::Logico(!ativo));
                    }
                    "Fina"|"Média"|"Grossa"|"Tracejada" => {
                        e.patch.insert("borderLineStyle".into(), Campo::Texto(opcao.into()));
                    }
                    _ => {}
                }
                e.patch.insert("borders".into(), Campo::Objeto(b));
            }
            // A paleta da aba vale para a cor dela (fundo, borda ou texto).
            Evento::CorPredefinida(valor) => e.cor_escolhida(valor.into()),
            Evento::MaisCores => e.mais_cores = !e.mais_cores,
            Evento::CorAlvo(alvo) => e.selecionar_alvo(alvo),
            Evento::CorSeletor(evento) => e.seletor_evento(evento),
            Evento::Grau(graus) => {
                let graus=graus.clamp(-90,90);
                e.angulo = graus.to_string();
                e.patch.insert("rotation".into(),Campo::Numero(graus as f64));
                e.patch.insert("textVertical".into(), Campo::Logico(false));
            }
            Evento::Mesclar(valor) => e.mesclar = valor,
            Evento::Confirmar | Evento::Cancelar => unreachable!(),
        }
        Task::none()
    }

}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn painel_seis_abas_nao_aplica_ate_confirmar() {
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,"Texto".into());
        let _=app.abrir_formatar_celulas(Aba::Fonte);
        assert_eq!(Aba::TODAS.len(),6);
        let _:Element<'_,Msg>=app.view_formatar_celulas();
        app.evento_formatar_celulas(Evento::FonteEstilo(3));
        assert!(!app.workbook.aba(0).unwrap().sheet
            .estilo_resolvido(0,0).unwrap_or_default().bold);
        app.evento_formatar_celulas(Evento::Confirmar);
        let estilo=app.workbook.aba(0).unwrap().sheet.estilo_resolvido(0,0).unwrap();
        assert!(estilo.bold && estilo.italic);
    }
    #[test]
    fn cores_hsv_e_transparencia_esperam_ok_e_podem_ser_canceladas() {
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,"Valor".into());
        app.formato_celula=Some(Estado::novo(&app,Aba::Preenchimento));
        app.evento_formatar_celulas(Evento::CorSeletor(seletor_cor::Evento::Escolhida(
            "#d93025".into())));
        app.evento_formatar_celulas(Evento::CorSeletor(seletor_cor::Evento::Alfa(0.5)));
        let escolhido=&app.formato_celula.as_ref().unwrap().cor_fundo;
        assert_eq!(escolhido.len(),9);
        assert_eq!(app.workbook.aba(0).unwrap().sheet
            .estilo_resolvido(0,0).unwrap_or_default().bg,None);
        app.evento_formatar_celulas(Evento::Cancelar);
        assert_eq!(app.workbook.aba(0).unwrap().sheet
            .estilo_resolvido(0,0).unwrap_or_default().bg,None);
        app.formato_celula=Some(Estado::novo(&app,Aba::Preenchimento));
        app.evento_formatar_celulas(Evento::CorSeletor(seletor_cor::Evento::Escolhida(
            "#458aff".into())));
        app.evento_formatar_celulas(Evento::Confirmar);
        assert_eq!(app.workbook.aba(0).unwrap().sheet
            .estilo_resolvido(0,0).unwrap().bg.as_deref(),Some("#458aff"));
    }
    #[test]
    fn borda_cor_personalizada_fica_no_rascunho_e_aplica_ao_confirmar() {
        let mut app=App::default();
        app.formato_celula=Some(Estado::novo(&app,Aba::Borda));
        app.evento_formatar_celulas(Evento::Borda("top"));
        app.evento_formatar_celulas(Evento::CorSeletor(seletor_cor::Evento::Escolhida(
            "#ff8800".into())));
        assert!(app.workbook.aba(0).unwrap().sheet
            .estilo_resolvido(0,0).unwrap_or_default().borders.is_none());
        app.evento_formatar_celulas(Evento::Confirmar);
        let borda=app.workbook.aba(0).unwrap().sheet
            .estilo_resolvido(0,0).unwrap().borders.unwrap();
        assert!(borda.top);
        assert_eq!(borda.color.as_deref(),Some("#ff8800"));
    }
    /// A criação do Scrollable no Iced 0.13 verifica que seu conteúdo
    /// vertical não usa Length::FILL. Isso regrediu ao abrir a aba Número.
    #[test]
    fn todas_as_abas_constroem_scrollable_sem_panic() {
        let mut app = App::default();
        app.formato_celula = Some(Estado::novo(&app, Aba::Numero));
        for (aba, _) in Aba::TODAS {
            app.evento_formatar_celulas(Evento::Aba(aba));
            let _: Element<'_, Msg> = app.view_formatar_celulas();
        }
        // Voltar à aba Número também precisa funcionar.
        app.evento_formatar_celulas(Evento::Aba(Aba::Numero));
        let _: Element<'_, Msg> = app.view_formatar_celulas();
    }

    #[test]
    fn cancelar_descarta_rascunho_e_customizacao() {
        let mut app=App::default();
        app.abrir_formatar_celulas(Aba::Alinhamento);
        app.evento_formatar_celulas(Evento::Grau(-30));
        app.evento_formatar_celulas(Evento::Texto(TextoCampo::CorFundo,"#d93025".into()));
        app.evento_formatar_celulas(Evento::Cancelar);
        assert!(app.workbook.aba(0).unwrap().sheet
            .estilo_resolvido(0,0).unwrap_or_default().rotation.is_none());
    }
}
