//! Configurações de um campo da tabela dinâmica (16/10/2026, meta da
//! v0.1.0), abertas pelo menu ▾ do campo e mostradas embaixo das áreas:
//!
//! - medida (Valores): "Resumir por" e "Mostrar valores como", com o campo
//!   base e o item base quando o modo pede;
//! - campo de Linhas ou Colunas: classificar (A a Z, Z a A, pelo valor de
//!   uma medida) e agrupar (ano, trimestre, mês, dia ou faixas numéricas).
//!
//! O motor já fazia tudo (`dinamica::exibicao`); aqui só a tela e o estado.
use super::*;
use feather_ui::icones::icone;
use pelicano::column;
use feathersheet_core::dinamica::dinamica_layout;
use feathersheet_core::dinamica::exibicao::{CampoBase, CampoExtra, Faixas, ItemBase, ParteData};

const OPERACOES: [AgregacaoOp; 7] = [
    AgregacaoOp::Soma, AgregacaoOp::Contagem, AgregacaoOp::Media, AgregacaoOp::Min,
    AgregacaoOp::Max, AgregacaoOp::DesvioPadrao, AgregacaoOp::Moda,
];

/// Até quantas linhas da origem a lista de itens base olha (a mesma conta
/// do filtro: não trava a tela em origem de milhões de linhas).
const LINHAS_DOS_ITENS: u32 = 20_000;
const MAX_ITENS: usize = 160;

/// Uma opção de lista suspensa: o valor e o texto que aparece.
#[derive(Debug, Clone, PartialEq)]
struct Escolha<T> {
    valor: T,
    nome: String,
}

impl<T> std::fmt::Display for Escolha<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.nome)
    }
}

/// O campo base do modo, quando ele tem um.
fn campo_do_modo(m: &MostrarComo) -> Option<CampoBase> {
    match m {
        MostrarComo::PercDe { campo, .. } | MostrarComo::DiferencaDe { campo, .. }
        | MostrarComo::PercDiferencaDe { campo, .. } | MostrarComo::TotalAcumuladoEm { campo }
        | MostrarComo::PercTotalAcumuladoEm { campo } | MostrarComo::ClassificarMenorMaior { campo }
        | MostrarComo::ClassificarMaiorMenor { campo } => Some(*campo),
        _ => None,
    }
}

fn item_do_modo(m: &MostrarComo) -> Option<&ItemBase> {
    match m {
        MostrarComo::PercDe { item, .. } | MostrarComo::DiferencaDe { item, .. }
        | MostrarComo::PercDiferencaDe { item, .. } => Some(item),
        _ => None,
    }
}

/// O mesmo modo com outro campo base (e o item volta a "(anterior)": o item
/// escolhido era de outro campo).
fn com_campo(m: &MostrarComo, campo: CampoBase) -> MostrarComo {
    let item = ItemBase::Anterior;
    match m {
        MostrarComo::PercDe { .. } => MostrarComo::PercDe { campo, item },
        MostrarComo::DiferencaDe { .. } => MostrarComo::DiferencaDe { campo, item },
        MostrarComo::PercDiferencaDe { .. } => MostrarComo::PercDiferencaDe { campo, item },
        MostrarComo::TotalAcumuladoEm { .. } => MostrarComo::TotalAcumuladoEm { campo },
        MostrarComo::PercTotalAcumuladoEm { .. } => MostrarComo::PercTotalAcumuladoEm { campo },
        MostrarComo::ClassificarMenorMaior { .. } => MostrarComo::ClassificarMenorMaior { campo },
        MostrarComo::ClassificarMaiorMenor { .. } => MostrarComo::ClassificarMaiorMenor { campo },
        outro => outro.clone(),
    }
}

fn com_item(m: &MostrarComo, item: ItemBase) -> MostrarComo {
    match m {
        MostrarComo::PercDe { campo, .. } => MostrarComo::PercDe { campo: *campo, item },
        MostrarComo::DiferencaDe { campo, .. } => MostrarComo::DiferencaDe { campo: *campo, item },
        MostrarComo::PercDiferencaDe { campo, .. } => MostrarComo::PercDiferencaDe { campo: *campo, item },
        outro => outro.clone(),
    }
}

/// Os modos na ordem do menu do Excel. Os que pedem campo base só entram
/// quando há campo em Linhas ou Colunas.
fn modos(campo: Option<CampoBase>, item: ItemBase) -> Vec<MostrarComo> {
    let mut v = vec![
        MostrarComo::Normal, MostrarComo::PercTotalGeral,
        MostrarComo::PercTotalColuna, MostrarComo::PercTotalLinha,
    ];
    if let Some(campo) = campo {
        v.extend([
            MostrarComo::PercDe { campo, item: item.clone() },
            MostrarComo::DiferencaDe { campo, item: item.clone() },
            MostrarComo::PercDiferencaDe { campo, item },
            MostrarComo::TotalAcumuladoEm { campo },
            MostrarComo::PercTotalAcumuladoEm { campo },
            MostrarComo::ClassificarMenorMaior { campo },
            MostrarComo::ClassificarMaiorMenor { campo },
        ]);
    }
    v.push(MostrarComo::Indice);
    v
}

fn nome_parte(p: ParteData) -> &'static str {
    match p {
        ParteData::Ano => tr!("Ano"),
        ParteData::Trimestre => tr!("Trimestre"),
        ParteData::Mes => tr!("Mês"),
        ParteData::Dia => tr!("Dia"),
    }
}

/// Número digitado com vírgula ou ponto.
fn numero(t: &str) -> Option<f64> {
    t.trim().replace(',', ".").parse::<f64>().ok().filter(|x| x.is_finite())
}

/// Número como se digita no idioma da interface.
fn numero_texto(x: f64) -> String {
    let t = if x.fract() == 0.0 && x.abs() < 1e15 { format!("{}", x as i64) } else { format!("{x}") };
    if crate::idioma::lingua() == crate::idioma::Lingua::En { t } else { t.replace('.', ",") }
}

/// Faixas de ~10 grupos com passo "redondo" (1, 2, 5 × 10ⁿ), do mínimo ao
/// máximo do campo, como o Excel sugere.
fn faixas_redondas(min: f64, max: f64) -> Faixas {
    if !(max > min) {
        return Faixas { inicio: min.floor(), fim: min.floor() + 1.0, intervalo: 1.0 };
    }
    let bruto = (max - min) / 10.0;
    let base = 10f64.powf(bruto.log10().floor());
    let passo = [1.0, 2.0, 5.0, 10.0].into_iter().map(|m| m * base)
        .find(|p| *p >= bruto).unwrap_or(10.0 * base);
    let inicio = (min / passo).floor() * passo;
    let fim = (max / passo).ceil() * passo;
    Faixas { inicio, fim: if fim > inicio { fim } else { inicio + passo }, intervalo: passo }
}

/// Os extras valendo para os campos de agora, prontos para mudar: daqui em
/// diante valem para estes campos.
fn extras_editaveis(e: &mut Estado) -> &mut Extras {
    let mut extras = avancado::extras_vigentes(e);
    extras.valores.resize(e.valores.len(), MostrarComo::Normal);
    extras.linhas.resize(e.linhas.len(), CampoExtra::default());
    extras.colunas.resize(e.colunas.len(), CampoExtra::default());
    e.extras_campos = (e.linhas.clone(), e.colunas.clone(), e.valores.iter().map(|m| m.coluna).collect());
    e.extras = extras;
    &mut e.extras
}

fn eixo_da_area(area: AreaDinamica) -> Option<EixoExtra> {
    match area {
        AreaDinamica::Linhas => Some(EixoExtra::Linhas),
        AreaDinamica::Colunas => Some(EixoExtra::Colunas),
        _ => None,
    }
}

impl Estado {
    fn coluna_do_eixo(&self, eixo: EixoExtra, posicao: usize) -> Option<usize> {
        if eixo == EixoExtra::Linhas { self.linhas.get(posicao).copied() } else { self.colunas.get(posicao).copied() }
    }

    pub(super) fn nome_da_coluna(&self, coluna: usize) -> String {
        self.nomes.get(coluna.saturating_sub(self.origem.c0 as usize)).cloned().unwrap_or_default()
    }

    /// O primeiro campo de Linhas (ou de Colunas): o base de partida.
    fn base_padrao(&self) -> Option<CampoBase> {
        if !self.linhas.is_empty() {
            Some(CampoBase { eixo: EixoExtra::Linhas, posicao: 0 })
        } else if !self.colunas.is_empty() {
            Some(CampoBase { eixo: EixoExtra::Colunas, posicao: 0 })
        } else {
            None
        }
    }
}

impl App {
    pub(super) fn evento_config_campo(&mut self, evento: Evento) {
        match evento {
            Evento::ConfigurarCampo(alvo) => {
                let Some(e) = self.dinamica.as_mut() else { return };
                e.ui.menu_item = None;
                e.ui.config_campo = alvo;
                e.ui.itens_base.clear();
                let Some((area, i)) = alvo else { return };
                let extras = avancado::extras_vigentes(e);
                match area {
                    AreaDinamica::Valores => {
                        if let Some(campo) = campo_do_modo(extras.mostrar_como(i)) {
                            self.ler_itens_base(campo);
                        }
                    }
                    _ => if let Some(eixo) = eixo_da_area(area) {
                        if let Some(Agrupamento::Faixas(f)) = extras.campo(eixo, i).agrupamento {
                            e.ui.faixa_txt = [numero_texto(f.inicio), numero_texto(f.fim), numero_texto(f.intervalo)];
                        }
                    },
                }
            }
            Evento::OperacaoEscolher(i, op) => {
                if let Some(m) = self.dinamica.as_mut().and_then(|e| e.valores.get_mut(i)) {
                    m.op = op;
                }
            }
            Evento::MostrarComo(i, modo) => {
                let Some(e) = self.dinamica.as_mut() else { return };
                let antes = campo_do_modo(avancado::extras_vigentes(e).mostrar_como(i));
                let novo = campo_do_modo(&modo);
                let extras = extras_editaveis(e);
                if let Some(v) = extras.valores.get_mut(i) {
                    *v = modo;
                }
                if novo != antes || e.ui.itens_base.is_empty() {
                    e.ui.itens_base.clear();
                    if let Some(campo) = novo {
                        self.ler_itens_base(campo);
                    }
                }
            }
            Evento::Ordenar(eixo, i, ordem) => {
                let Some(e) = self.dinamica.as_mut() else { return };
                let extras = extras_editaveis(e);
                let lista = if eixo == EixoExtra::Linhas { &mut extras.linhas } else { &mut extras.colunas };
                if let Some(c) = lista.get_mut(i) {
                    c.ordenacao = ordem;
                }
            }
            Evento::AgruparCampo(eixo, i, agrupamento) => {
                // Faixas sem intervalo = "escolher sozinho" pelo mínimo e máximo.
                let agrupamento = match agrupamento {
                    Some(Agrupamento::Faixas(f)) if !(f.intervalo > 0.0) => {
                        Some(Agrupamento::Faixas(self.faixas_automaticas(eixo, i)))
                    }
                    outro => outro,
                };
                let Some(e) = self.dinamica.as_mut() else { return };
                if let Some(Agrupamento::Faixas(f)) = agrupamento {
                    e.ui.faixa_txt = [numero_texto(f.inicio), numero_texto(f.fim), numero_texto(f.intervalo)];
                }
                let extras = extras_editaveis(e);
                let lista = if eixo == EixoExtra::Linhas { &mut extras.linhas } else { &mut extras.colunas };
                if let Some(c) = lista.get_mut(i) {
                    c.agrupamento = agrupamento;
                }
            }
            Evento::FaixaTexto(k, texto) => {
                let Some(e) = self.dinamica.as_mut() else { return };
                let Some((area, i)) = e.ui.config_campo else { return };
                let Some(eixo) = eixo_da_area(area) else { return };
                if let Some(t) = e.ui.faixa_txt.get_mut(k) {
                    *t = texto;
                }
                // Só vale quando os três formam faixas de verdade.
                let [a, b, c] = &e.ui.faixa_txt;
                let (Some(inicio), Some(fim), Some(intervalo)) = (numero(a), numero(b), numero(c)) else { return };
                if !(intervalo > 0.0 && fim > inicio) || (fim - inicio) / intervalo > 10_000.0 {
                    return;
                }
                let extras = extras_editaveis(e);
                let lista = if eixo == EixoExtra::Linhas { &mut extras.linhas } else { &mut extras.colunas };
                if let Some(c) = lista.get_mut(i) {
                    c.agrupamento = Some(Agrupamento::Faixas(Faixas { inicio, fim, intervalo }));
                }
            }
            Evento::TotaisGerais { linhas, ligado } => {
                let Some(e) = self.dinamica.as_mut() else { return };
                let extras = extras_editaveis(e);
                if linhas { extras.totais_gerais_linhas = ligado } else { extras.totais_gerais_colunas = ligado }
            }
            _ => {}
        }
    }

    /// Lê os itens do campo base (com o agrupamento dele), na ordem natural,
    /// para a lista "Item base".
    fn ler_itens_base(&mut self, campo: CampoBase) {
        let Some(e) = &self.dinamica else { return };
        let Some(coluna) = e.coluna_do_eixo(campo.eixo, campo.posicao) else { return };
        let extra = avancado::extras_vigentes(e).campo(campo.eixo, campo.posicao);
        let ultima = e.origem.r1.min(e.origem.r0.saturating_add(LINHAS_DOS_ITENS));
        let mut vistos: std::collections::HashMap<String, Option<f64>> = std::collections::HashMap::new();
        for r in (e.origem.r0 + 1)..=ultima {
            if vistos.len() >= MAX_ITENS { break; }
            let v = self.workbook.valor(e.folha_origem, r, coluna as u32);
            let (rotulo, ordem) = dinamica_layout::rotulo_do_item(&v, &extra, e.agrupar_data);
            vistos.entry(rotulo).or_insert(ordem);
        }
        let mut itens: Vec<(String, Option<f64>)> = vistos.into_iter().collect();
        itens.sort_by(|(a, oa), (b, ob)| match (oa, ob) {
            (Some(x), Some(y)) => x.total_cmp(y),
            _ => feathersheet_core::dinamica::exibicao::comparar_natural(a, b),
        });
        if let Some(e) = &mut self.dinamica {
            e.ui.itens_base = itens.into_iter().map(|(n, _)| n).collect();
        }
    }

    /// Faixas sugeridas pelo mínimo e o máximo numéricos do campo.
    fn faixas_automaticas(&self, eixo: EixoExtra, posicao: usize) -> Faixas {
        let Some(e) = &self.dinamica else { return faixas_redondas(0.0, 0.0) };
        let Some(coluna) = e.coluna_do_eixo(eixo, posicao) else { return faixas_redondas(0.0, 0.0) };
        let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
        for r in (e.origem.r0 + 1)..=e.origem.r1 {
            if let Value::Number(x) = self.workbook.valor(e.folha_origem, r, coluna as u32) {
                min = min.min(x);
                max = max.max(x);
            }
        }
        if min > max { faixas_redondas(0.0, 0.0) } else { faixas_redondas(min, max) }
    }

    /// O quadro das configurações do campo, embaixo das quatro áreas.
    pub(super) fn view_config_campo<'a>(&'a self, e: &'a Estado, area: AreaDinamica, i: usize) -> Element<'a, Msg> {
        let extras = avancado::extras_vigentes(e);
        let rotulo = |t: &str| text(t.to_owned()).size(11).color(COR_TEXTO_MUTED());
        let mut corpo = column(Vec::<pelicano::Element<_>>::new()).spacing(6);
        let titulo: String;
        match area {
            AreaDinamica::Valores => {
                let Some(medida) = e.valores.get(i) else { return Space::new(0.0, 0).into() };
                titulo = trf!("{} de {}", nome_operacao(medida.op), e.nome_da_coluna(medida.coluna));
                let ops: Vec<Escolha<AgregacaoOp>> = OPERACOES.iter()
                    .map(|&op| Escolha { valor: op, nome: nome_operacao(op).to_owned() }).collect();
                let op_atual = ops.iter().find(|o| o.valor == medida.op).cloned();
                corpo = corpo.push(rotulo(tr!("Resumir por")));
                corpo = corpo.push(crate::compat::pick_list_vec(ops, op_atual, move |o: Escolha<AgregacaoOp>|
                    Msg::Dinamica(Evento::OperacaoEscolher(i, o.valor)))
                    .size(12).padding([5, 8]).width(Length::FILL));

                let atual = extras.mostrar_como(i).clone();
                let campo = campo_do_modo(&atual).or_else(|| e.base_padrao());
                let item = item_do_modo(&atual).cloned().unwrap_or(ItemBase::Anterior);
                let lista: Vec<Escolha<MostrarComo>> = modos(campo, item).into_iter()
                    .map(|m| Escolha { nome: tr!(m.rotulo()).to_owned(), valor: m }).collect();
                let selecionado = lista.iter().find(|o| o.valor == atual).cloned();
                corpo = corpo.push(rotulo(tr!("Mostrar valores como")));
                corpo = corpo.push(crate::compat::pick_list_vec(lista, selecionado, move |o: Escolha<MostrarComo>|
                    Msg::Dinamica(Evento::MostrarComo(i, o.valor)))
                    .size(12).padding([5, 8]).width(Length::FILL));

                if let Some(base) = campo_do_modo(&atual) {
                    let campos: Vec<Escolha<CampoBase>> = e.linhas.iter().enumerate()
                        .map(|(p, &c)| (EixoExtra::Linhas, p, c))
                        .chain(e.colunas.iter().enumerate().map(|(p, &c)| (EixoExtra::Colunas, p, c)))
                        .map(|(eixo, posicao, c)| Escolha { valor: CampoBase { eixo, posicao }, nome: e.nome_da_coluna(c) })
                        .collect();
                    let campo_atual = campos.iter().find(|o| o.valor == base).cloned();
                    let modo = atual.clone();
                    corpo = corpo.push(rotulo(tr!("Campo base")));
                    corpo = corpo.push(crate::compat::pick_list_vec(campos, campo_atual, move |o: Escolha<CampoBase>|
                        Msg::Dinamica(Evento::MostrarComo(i, com_campo(&modo, o.valor))))
                        .size(12).padding([5, 8]).width(Length::FILL));
                }
                if let Some(item_atual) = item_do_modo(&atual) {
                    let mut itens = vec![
                        Escolha { valor: ItemBase::Anterior, nome: tr!("(anterior)").to_owned() },
                        Escolha { valor: ItemBase::Seguinte, nome: tr!("(seguinte)").to_owned() },
                    ];
                    itens.extend(e.ui.itens_base.iter().map(|n| Escolha { valor: ItemBase::Item { nome: n.clone() }, nome: n.clone() }));
                    let item_sel = itens.iter().find(|o| &o.valor == item_atual).cloned();
                    let modo = atual.clone();
                    corpo = corpo.push(rotulo(tr!("Item base")));
                    corpo = corpo.push(crate::compat::pick_list_vec(itens, item_sel, move |o: Escolha<ItemBase>|
                        Msg::Dinamica(Evento::MostrarComo(i, com_item(&modo, o.valor))))
                        .size(12).padding([5, 8]).width(Length::FILL));
                }
                if e.linhas.is_empty() && e.colunas.is_empty() {
                    corpo = corpo.push(text(tr!("Ponha um campo em Linhas ou Colunas para ver \"% de\", \"Diferença de\", acumulados e classificação."))
                        .size(11).color(COR_TEXTO_MUTED()));
                }
            }
            AreaDinamica::Linhas | AreaDinamica::Colunas => {
                let eixo = eixo_da_area(area).expect("Linhas ou Colunas");
                let Some(coluna) = e.coluna_do_eixo(eixo, i) else { return Space::new(0.0, 0).into() };
                titulo = e.nome_da_coluna(coluna);
                let extra = extras.campo(eixo, i);

                let mut ordens = vec![
                    Escolha { valor: Ordenacao::Padrao, nome: tr!("Padrão").to_owned() },
                    Escolha { valor: Ordenacao::Crescente, nome: tr!("Crescente (A a Z)").to_owned() },
                    Escolha { valor: Ordenacao::Decrescente, nome: tr!("Decrescente (Z a A)").to_owned() },
                ];
                for (k, m) in e.valores.iter().enumerate() {
                    let medida = trf!("{} de {}", nome_operacao(m.op), e.nome_da_coluna(m.coluna));
                    ordens.push(Escolha { valor: Ordenacao::PorValor { medida: k, crescente: false },
                        nome: trf!("Pelo valor de {medida}, maior primeiro") });
                    ordens.push(Escolha { valor: Ordenacao::PorValor { medida: k, crescente: true },
                        nome: trf!("Pelo valor de {medida}, menor primeiro") });
                }
                let ordem_atual = ordens.iter().find(|o| o.valor == extra.ordenacao).cloned();
                corpo = corpo.push(rotulo(tr!("Classificar")));
                corpo = corpo.push(crate::compat::pick_list_vec(ordens, ordem_atual, move |o: Escolha<Ordenacao>|
                    Msg::Dinamica(Evento::Ordenar(eixo, i, o.valor)))
                    .size(12).padding([5, 8]).width(Length::FILL));

                let faixas_atuais = match extra.agrupamento {
                    Some(Agrupamento::Faixas(f)) => f,
                    _ => Faixas { inicio: 0.0, fim: 0.0, intervalo: 0.0 },
                };
                let mut grupos = vec![Escolha { valor: None, nome: tr!("Sem agrupamento").to_owned() }];
                grupos.extend([ParteData::Ano, ParteData::Trimestre, ParteData::Mes, ParteData::Dia].map(|parte|
                    Escolha { valor: Some(Agrupamento::Data { parte }), nome: trf!("Datas por {}", nome_parte(parte).to_lowercase()) }));
                grupos.push(Escolha { valor: Some(Agrupamento::Faixas(faixas_atuais)), nome: tr!("Números em faixas").to_owned() });
                let grupo_atual = grupos.iter().find(|o| o.valor == extra.agrupamento).cloned();
                corpo = corpo.push(rotulo(tr!("Agrupar")));
                corpo = corpo.push(crate::compat::pick_list_vec(grupos, grupo_atual, move |o: Escolha<Option<Agrupamento>>|
                    Msg::Dinamica(Evento::AgruparCampo(eixo, i, o.valor)))
                    .size(12).padding([5, 8]).width(Length::FILL));
                if matches!(extra.agrupamento, Some(Agrupamento::Faixas(_))) {
                    let campo = |k: usize, nome: &'static str| -> Element<'a, Msg> {
                        column![
                            rotulo(nome),
                            text_input("0", &e.ui.faixa_txt[k])
                                .on_input(move |t| Msg::Dinamica(Evento::FaixaTexto(k, t)))
                                .padding(5).size(12),
                        ].spacing(2).width(Length::FILL).into()
                    };
                    corpo = corpo.push(row![
                        campo(0, tr!("Começa em")), campo(1, tr!("Termina em")), campo(2, tr!("De quanto em quanto")),
                    ].spacing(6));
                }
            }
            _ => return Space::new(0.0, 0).into(),
        }
        let cabecalho = row![
            text(titulo).size(12.5).width(Length::FILL),
            button(icone("x", 13.0)).padding([3, 5]).style(estilo_botao_pequeno(false))
                .on_press(Msg::Dinamica(Evento::ConfigurarCampo(None))),
        ].spacing(6).align(pelicano::Align::Center);
        container(column![cabecalho, corpo].spacing(8)).padding(8).width(Length::FILL)
            .style(|_| pelicano::Style {
                background: Some(pelicano::Background::Color(COR_PAINEL())),
                border: pelicano::Border { color: COR_BORDA_SUAVE(), width: 1.0, radius: [8.0; 4] },
                ..Default::default()
            })
            .into()
    }
}

/// Põe nas colunas de valor do resultado (`area`, cabeçalho na 1ª linha) o
/// formato que o "Mostrar valores como" pede: porcentagem, inteiro da
/// classificação, duas casas do índice. A coluna que voltou a "Sem cálculo"
/// perde a porcentagem que tinha ganhado (senão 1.234 viraria 123.400%).
pub(super) fn formatar_valores(
    sheet: &mut feathersheet_core::planilha::sheet::Sheet,
    area: &Rect,
    resultado: &dinamica_layout::Resultado,
    extras: &Extras,
    n_medidas: usize,
) {
    use feathersheet_core::celulas::estilo_parcial::{Campo, EstiloParcial};
    use feathersheet_core::celulas::style::NumberFormat;
    if area.r1 <= area.r0 {
        return;
    }
    let largura = area.c1 - area.c0 + 1;
    for j in 0..largura {
        let c = area.c0 + j;
        let dados = Rect::new(area.r0 + 1, c, area.r1, c);
        let mut patch = EstiloParcial::new();
        match dinamica_layout::formato_da_coluna(resultado, extras, n_medidas, j as usize) {
            Some("0,00%") => {
                patch.insert("format".into(), Campo::Texto("percent".into()));
                patch.insert("decimals".into(), Campo::Numero(2.0));
            }
            Some("0") => {
                patch.insert("format".into(), Campo::Texto("integer".into()));
            }
            Some(_) => {
                patch.insert("format".into(), Campo::Texto("number".into()));
                patch.insert("decimals".into(), Campo::Numero(2.0));
            }
            None => {
                let percentual = sheet.estilo_resolvido(area.r0 + 1, c)
                    .is_some_and(|s| s.format == NumberFormat::Percent);
                if !percentual || (j as usize) < resultado.colunas_de_rotulo {
                    continue;
                }
                patch.insert("format".into(), Campo::Nulo);
                patch.insert("decimals".into(), Campo::Nulo);
            }
        }
        sheet.set_style(&dados, &patch);
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn app_com_vendas() -> App {
        let mut app = App::default();
        app.workbook.editar(0, |s| {
            s.set_cell(0, 0, "Região"); s.set_cell(0, 1, "Valor");
            s.set_cell(1, 0, "Norte"); s.set_cell(1, 1, "10");
            s.set_cell(2, 0, "Sul"); s.set_cell(2, 1, "30");
            s.set_cell(3, 0, "Norte"); s.set_cell(3, 1, "20");
        });
        app.fim_selecao = (3, 1);
        app.evento_dinamica(Evento::Abrir);
        crate::app::dinamica_ui::campos_de_antes(&mut app);
        app
    }

    #[test]
    fn faixas_redondas_cobrem_o_minimo_e_o_maximo() {
        let f = faixas_redondas(3.0, 97.0);
        assert_eq!(f.intervalo, 10.0);
        assert!(f.inicio <= 3.0 && f.fim >= 97.0);
        let f = faixas_redondas(0.0, 1234.0);
        assert_eq!(f.intervalo, 200.0);
    }

    #[test]
    fn mostrar_como_fica_na_medida_e_segue_ao_trocar_campos() {
        let mut app = app_com_vendas();
        app.evento_dinamica(Evento::MostrarComo(0, MostrarComo::PercTotalGeral));
        let e = app.dinamica.as_ref().unwrap();
        assert_eq!(*avancado::extras_vigentes(e).mostrar_como(0), MostrarComo::PercTotalGeral);
        // Uma segunda medida entra antes: o "% do total" acompanha a primeira.
        let col = e.valores[0].coluna;
        let e = app.dinamica.as_mut().unwrap();
        e.valores.insert(0, MedidaDinamica { coluna: col, op: AgregacaoOp::Contagem });
        let x = avancado::extras_vigentes(app.dinamica.as_ref().unwrap());
        assert_eq!(x.valores.len(), 2);
        assert_eq!(x.valores.iter().filter(|m| **m == MostrarComo::PercTotalGeral).count(), 1);
    }

    #[test]
    fn campo_base_que_saiu_volta_a_sem_calculo() {
        let mut app = app_com_vendas();
        let campo = CampoBase { eixo: EixoExtra::Linhas, posicao: 0 };
        app.evento_dinamica(Evento::MostrarComo(0, MostrarComo::TotalAcumuladoEm { campo }));
        assert!(!app.dinamica.as_ref().unwrap().ui.itens_base.is_empty(), "lê os itens do campo base");
        app.dinamica.as_mut().unwrap().linhas.clear();
        let x = avancado::extras_vigentes(app.dinamica.as_ref().unwrap());
        assert_eq!(x.valores, vec![MostrarComo::Normal]);
    }

    #[test]
    fn faixa_digitada_so_vale_quando_forma_faixas() {
        let mut app = app_com_vendas();
        app.evento_dinamica(Evento::ConfigurarCampo(Some((AreaDinamica::Linhas, 0))));
        app.evento_dinamica(Evento::AgruparCampo(EixoExtra::Linhas, 0,
            Some(Agrupamento::Faixas(Faixas { inicio: 0.0, fim: 0.0, intervalo: 0.0 }))));
        let e = app.dinamica.as_ref().unwrap();
        assert!(matches!(avancado::extras_vigentes(e).campo(EixoExtra::Linhas, 0).agrupamento,
            Some(Agrupamento::Faixas(f)) if f.intervalo > 0.0));
        app.evento_dinamica(Evento::FaixaTexto(2, "-5".into()));
        app.evento_dinamica(Evento::FaixaTexto(0, "0".into()));
        app.evento_dinamica(Evento::FaixaTexto(1, "100".into()));
        let antes = avancado::extras_vigentes(app.dinamica.as_ref().unwrap()).campo(EixoExtra::Linhas, 0);
        assert!(matches!(antes.agrupamento, Some(Agrupamento::Faixas(f)) if f.intervalo > 0.0), "intervalo negativo não vale");
        app.evento_dinamica(Evento::FaixaTexto(2, "25".into()));
        let depois = avancado::extras_vigentes(app.dinamica.as_ref().unwrap()).campo(EixoExtra::Linhas, 0);
        assert_eq!(depois.agrupamento, Some(Agrupamento::Faixas(Faixas { inicio: 0.0, fim: 100.0, intervalo: 25.0 })));
    }

    #[test]
    fn totais_gerais_desligam_separados() {
        let mut app = app_com_vendas();
        app.evento_dinamica(Evento::TotaisGerais { linhas: false, ligado: false });
        let x = avancado::extras_vigentes(app.dinamica.as_ref().unwrap());
        assert!(x.totais_gerais_linhas && !x.totais_gerais_colunas);
    }
}
