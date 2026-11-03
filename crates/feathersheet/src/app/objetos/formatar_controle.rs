//! "Formatar controle" (botão direito num controle de formulário), como no Excel: a aba
//! Controle com os campos de cada tipo — botão de girar e barra de rolagem: valor atual,
//! mínimo, máximo, alteração incremental, mudança de página (só a barra) e vínculo da célula;
//! caixa e opção: marcada/desmarcada/mista; lista e combinação: intervalo de entrada e
//! linhas suspensas. OK grava no controle (um Ctrl+Z) e o valor atual vai para a célula.
use super::*;
use feathersheet_io::objetos_aba::{ControleFormulario, TipoControleForm as T};
use pelicano::column;

/// Abas do diálogo, na ordem do Excel (abre na Controle).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AbaControle { Fonte, Alinhamento, CoresELinhas, Tamanho, Protecao, Propriedades, TextoAlt, #[default] Controle }

/// As abas que o Excel mostra para cada tipo: o botão tem Fonte/Alinhamento e não tem Controle;
/// caixa e opção têm Cores e linhas.
fn abas(tipo: T) -> Vec<(AbaControle, &'static str)> {
    let mut v = Vec::new();
    match tipo {
        T::Botao => v.extend([(AbaControle::Fonte, "Fonte"), (AbaControle::Alinhamento, "Alinhamento")]),
        T::Caixa | T::Opcao => v.push((AbaControle::CoresELinhas, "Cores e linhas")),
        _ => {}
    }
    v.extend([(AbaControle::Tamanho, "Tamanho"), (AbaControle::Protecao, "Proteção"), (AbaControle::Propriedades, "Propriedades"),
        (AbaControle::TextoAlt, "Texto Alt.")]);
    if tipo != T::Botao { v.push((AbaControle::Controle, "Controle")); }
    v
}

/// Qual cor a paleta muda.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlvoCor { Fundo, Linha, Texto }

const CORES_FUNDO: [&str; 8] = ["#ffffff", "#ffff99", "#ccffcc", "#ccecff", "#d9d9d9", "#ffcc99", "#ff9999", "#4472c4"];
const CORES_LINHA: [&str; 6] = ["#000000", "#808080", "#4472c4", "#c00000", "#00b050", "#ffc000"];
const CORES_TEXTO: [&str; 6] = ["#000000", "#ffffff", "#c00000", "#4472c4", "#00b050", "#808080"];

/// Caixas de marcar do diálogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcao { Bloqueado, BloquearTexto, Imprimir, Sombreado, Negrito, Italico }

/// O que está sendo digitado no diálogo, como texto (o OK confere e converte).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rascunho {
    pub id: u64,
    pub tipo: Option<T>,
    pub aba: AbaControle,
    pub valor: String,
    pub min: String,
    pub max: String,
    pub passo: String,
    pub pagina: String,
    pub celula: String,
    pub faixa: String,
    pub linhas: String,
    pub marcado: u8,
    pub altura: String,
    pub largura: String,
    pub alt: String,
    pub bloqueado: bool,
    pub bloquear_texto: bool,
    pub imprimir: bool,
    pub sombreado: bool,
    pub fundo: Option<String>,
    pub linha: Option<String>,
    pub cor_texto: Option<String>,
    pub fonte_tam: String,
    pub negrito: bool,
    pub italico: bool,
    pub alinhar: modelo::AlinharTexto,
    pub posicao: u8,
    /// Como o diálogo abriu: o OK só escreve na célula ligada o que mudou.
    pub valor_original: String,
    pub marcado_original: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Campo { Valor, Min, Max, Passo, Pagina, Celula, Faixa, Linhas, Altura, Largura, Alt, FonteTam }

/// O Excel aceita de 0 a 30000 nos números do controle.
const TETO: i64 = 30_000;
/// Pixels lógicos (96 ppp) por centímetro: o Tamanho do Excel em português é em cm.
const PX_CM: f32 = 96.0 / 2.54;

fn numero(t: &str) -> Result<Option<i64>, String> {
    let t = t.trim();
    if t.is_empty() { return Ok(None); }
    let n: i64 = t.parse().map_err(|_| format!("\"{t}\" não é um número inteiro"))?;
    if !(0..=TETO).contains(&n) { return Err(format!("{n} fora de 0 a {TETO}")); }
    Ok(Some(n))
}

fn cm(px: f32) -> String { format!("{:.2}", px / PX_CM).replace('.', ",") }

/// Centímetros (vírgula ou ponto) para px; de 0,05 a 200 cm.
fn de_cm(t: &str) -> Result<f32, String> {
    let v: f32 = t.trim().trim_end_matches("cm").trim().replace(',', ".").parse().map_err(|_| format!("\"{}\" não é uma medida em cm", t.trim()))?;
    if !(0.05..=200.0).contains(&v) { return Err(format!("{} cm fora de 0,05 a 200", t.trim())); }
    Ok(v * PX_CM)
}

impl Rascunho {
    /// `tamanho` = largura e altura atuais em px.
    pub fn de(o: &ObjetoSolto, c: &ControleFormulario, tamanho: (f32, f32)) -> Self {
        let n = |v: Option<i64>| v.map_or(String::new(), |v| v.to_string());
        let (id, alt) = (o.id, c.texto_alt.as_deref());
        let f = o.fonte.clone().unwrap_or_default();
        Self { id, tipo: Some(c.tipo), aba: if c.tipo == T::Botao { AbaControle::Fonte } else { AbaControle::Controle },
            fundo: c.fundo.clone(), linha: c.linha.clone(), cor_texto: f.cor.clone(),
            fonte_tam: f.tamanho.map_or(String::new(), |t| t.to_string().replace('.', ",")), negrito: f.negrito, italico: f.italico,
            alinhar: o.alinhar, posicao: c.posicao, valor: n(c.valor), min: n(c.min), max: n(c.max), passo: n(c.passo), pagina: n(c.pagina),
            celula: c.celula.clone().unwrap_or_default(), faixa: c.faixa.clone().unwrap_or_default(),
            linhas: c.linhas.map_or(String::new(), |v| v.to_string()), marcado: c.marcado.unwrap_or(0),
            largura: cm(tamanho.0), altura: cm(tamanho.1), alt: alt.unwrap_or_default().to_owned(),
            bloqueado: !c.desbloqueado, bloquear_texto: !c.texto_desbloqueado, imprimir: !c.nao_imprimir, sombreado: !c.sem_3d,
            valor_original: n(c.valor), marcado_original: c.marcado.unwrap_or(0) }
    }

    pub fn campo(&mut self, campo: Campo, texto: String) {
        *match campo {
            Campo::Valor => &mut self.valor, Campo::Min => &mut self.min, Campo::Max => &mut self.max,
            Campo::Passo => &mut self.passo, Campo::Pagina => &mut self.pagina, Campo::Celula => &mut self.celula,
            Campo::Faixa => &mut self.faixa, Campo::Linhas => &mut self.linhas,
            Campo::Altura => &mut self.altura, Campo::Largura => &mut self.largura, Campo::Alt => &mut self.alt,
            Campo::FonteTam => &mut self.fonte_tam,
        } = texto;
    }

    pub fn opcao(&mut self, o: Opcao, v: bool) {
        *match o {
            Opcao::Bloqueado => &mut self.bloqueado, Opcao::BloquearTexto => &mut self.bloquear_texto,
            Opcao::Imprimir => &mut self.imprimir, Opcao::Sombreado => &mut self.sombreado,
            Opcao::Negrito => &mut self.negrito, Opcao::Italico => &mut self.italico,
        } = v;
    }

    pub fn cor(&mut self, alvo: AlvoCor, cor: Option<String>) {
        *match alvo { AlvoCor::Fundo => &mut self.fundo, AlvoCor::Linha => &mut self.linha, AlvoCor::Texto => &mut self.cor_texto } = cor;
    }

    /// A fonte nova (tamanho de 4 a 96 pt); `None` = a padrão.
    pub fn fonte(&self, atual: Option<modelo::FonteTexto>) -> Result<Option<modelo::FonteTexto>, String> {
        let t = self.fonte_tam.trim().replace(',', ".");
        let tamanho = if t.is_empty() { None } else {
            let v: f32 = t.parse().map_err(|_| format!("\"{}\" não é um tamanho de fonte", self.fonte_tam.trim()))?;
            if !(4.0..=96.0).contains(&v) { return Err(format!("Fonte de {t} pt fora de 4 a 96")); }
            Some(v)
        };
        let mut f = atual.unwrap_or_default();
        (f.tamanho, f.negrito, f.italico, f.cor) = (tamanho, self.negrito, self.italico, self.cor_texto.clone());
        Ok((f != modelo::FonteTexto::default()).then_some(f))
    }

    /// Largura e altura novas em px.
    pub fn tamanho(&self) -> Result<(f32, f32), String> { Ok((de_cm(&self.largura)?, de_cm(&self.altura)?)) }

    /// O controle com o que foi digitado; `Err` diz o campo errado.
    pub fn aplicar(&self, c: &ControleFormulario) -> Result<ControleFormulario, String> {
        let mut novo = c.clone();
        let texto = |t: &str| Some(t.trim().to_owned()).filter(|t| !t.is_empty());
        novo.celula = texto(&self.celula);
        novo.desbloqueado = !self.bloqueado;
        novo.texto_desbloqueado = !self.bloquear_texto;
        novo.nao_imprimir = !self.imprimir;
        novo.sem_3d = !self.sombreado;
        novo.texto_alt = texto(&self.alt);
        (novo.fundo, novo.linha, novo.posicao) = (self.fundo.clone(), self.linha.clone(), self.posicao);
        match c.tipo {
            T::Girar | T::Rolagem => {
                let (min, max) = (numero(&self.min)?, numero(&self.max)?);
                if let (Some(a), Some(b)) = (min, max) { if a > b { return Err("O mínimo é maior que o máximo".into()); } }
                novo.min = min;
                novo.max = max;
                novo.passo = numero(&self.passo)?;
                if c.tipo == T::Rolagem { novo.pagina = numero(&self.pagina)?; }
                // O valor atual fica entre o mínimo e o máximo, como o Excel faz.
                // Mínimo sem máximo (ou o contrário) pode inverter a faixa padrão: `clamp` não aceita.
                let (lo, hi) = (min.unwrap_or(0), max.unwrap_or(100));
                novo.valor = numero(&self.valor)?.map(|v| v.clamp(lo.min(hi), hi.max(lo)));
            }
            T::Caixa | T::Opcao => novo.marcado = Some(self.marcado),
            T::Lista | T::Combo => {
                novo.faixa = texto(&self.faixa);
                if c.tipo == T::Combo { novo.linhas = numero(&self.linhas)?.map(|v| v as u32); }
            }
            T::Botao | T::Rotulo | T::Grupo => {}
        }
        Ok(novo)
    }
}

fn ev(e: Evento) -> Msg { Msg::Objetos(e) }

fn linha<'a>(rotulo: &'static str, valor: &'a str, campo: Campo, ativo: bool) -> Element<'a, Msg> {
    let mut c = text_input("", valor).size(13).padding([5, 8]).width(Length::Fixed(150.0));
    if ativo { c = c.on_input(move |t| ev(Evento::ControleCampo(campo, t))).on_submit(ev(Evento::ControleOk)); }
    row![text(tr!(rotulo)).size(13).width(Length::Fixed(170.0)).color(if ativo { COR_TEXTO() } else { COR_TEXTO_MUTED() }), c]
        .spacing(10).align(pelicano::Align::Center).into()
}

fn marcar(rotulo: &'static str, v: bool, o: Opcao) -> Element<'static, Msg> {
    pelicano::checkbox(tr!(rotulo), v).on_toggle(move |v| ev(Evento::ControleOpcao(o, v))).size(12.5).into()
}

/// "Automático" + quadradinhos de cor; o escolhido fica marcado.
fn paleta(alvo: AlvoCor, atual: Option<&str>, cores: &'static [&'static str]) -> Element<'static, Msg> {
    let mut linha = row![button(text(tr!("Automático")).size(12)).padding([4, 8]).style(estilo_botao_pequeno(atual.is_none()))
        .on_press(ev(Evento::ControleCor(alvo, None)))].spacing(5).align(pelicano::Align::Center);
    for &c in cores {
        let cor = super::camada::cor_hex(c, 0.0);
        let escolhida = atual.is_some_and(|a| a.eq_ignore_ascii_case(c));
        linha = linha.push(button(container(Space::new(Length::Fixed(16.0), Length::Fixed(16.0))).style(move |_: &pelicano::Theme| pelicano::Style {
                background: Some(pelicano::Background::Color(cor)),
                border: pelicano::Border { color: if escolhida { COR_AZUL_ACENTO() } else { COR_TEXTO_MUTED() }, width: if escolhida { 2.0 } else { 1.0 }, radius: [3.0; 4] },
                ..Default::default()
            }))
            .padding(2).style(estilo_botao_pequeno(escolhida)).on_press(ev(Evento::ControleCor(alvo, Some(c.to_owned())))));
    }
    linha.into()
}

/// Botões lado a lado, um escolhido (alinhamento, posicionamento).
fn escolha<V: Copy + PartialEq + 'static>(opcoes: &[(V, &'static str)], atual: V, msg: fn(V) -> Evento) -> Element<'static, Msg> {
    let mut l = row![].spacing(5);
    for &(v, rotulo) in opcoes {
        l = l.push(button(text(tr!(rotulo)).size(12)).padding([5, 10]).style(estilo_botao_pequeno(v == atual)).on_press(ev(msg(v))));
    }
    l.into()
}

fn nota(t: &'static str) -> Element<'static, Msg> { text(tr!(t)).size(12).color(COR_TEXTO_MUTED()).into() }

impl App {
    fn aba_controle<'a>(&self, r: &'a Rascunho, tipo: T) -> Element<'a, Msg> {
        let marca = |rotulo: &'static str, v: u8| -> Element<'static, Msg> {
            button(row![feather_ui::icones::icone(if r.marcado == v { "circle-check" } else { "circle" }, 15.0), text(tr!(rotulo)).size(13)]
                .spacing(8).align(pelicano::Align::Center))
                .padding([4, 8]).style(estilo_item_menu(r.marcado == v, true)).on_press(ev(Evento::ControleMarcado(v))).into()
        };
        let mut campos = column![].spacing(9);
        match tipo {
            T::Girar | T::Rolagem => {
                campos = campos
                    .push(linha("Valor atual:", &r.valor, Campo::Valor, true))
                    .push(linha("Valor mínimo:", &r.min, Campo::Min, true))
                    .push(linha("Valor máximo:", &r.max, Campo::Max, true))
                    .push(linha("Alteração incremental:", &r.passo, Campo::Passo, true))
                    // No botão de girar a mudança de página não existe (o Excel deixa apagada).
                    .push(linha("Mudança de página:", &r.pagina, Campo::Pagina, tipo == T::Rolagem))
                    .push(linha("Vínculo da célula:", &r.celula, Campo::Celula, true));
            }
            T::Caixa | T::Opcao => {
                campos = campos.push(text(tr!("Valor")).size(13))
                    .push(row![marca("Desmarcada", 0), marca("Marcada", 1)].push_maybe((tipo == T::Caixa).then(|| marca("Mista", 2))).spacing(6))
                    .push(linha("Vínculo da célula:", &r.celula, Campo::Celula, true));
            }
            T::Lista | T::Combo => {
                campos = campos
                    .push(linha("Intervalo de entrada:", &r.faixa, Campo::Faixa, true))
                    .push(linha("Vínculo da célula:", &r.celula, Campo::Celula, true));
                if tipo == T::Combo { campos = campos.push(linha("Linhas suspensas:", &r.linhas, Campo::Linhas, true)); }
            }
            T::Botao | T::Rotulo | T::Grupo => {
                campos = campos.push(nota("Este controle não tem valor. Use Editar texto ou Atribuir macro no botão direito."));
            }
        }
        if !matches!(tipo, T::Botao | T::Rotulo) { campos = campos.push(marcar("Sombreamento 3D", r.sombreado, Opcao::Sombreado)); }
        campos.into()
    }

    /// O diálogo aberto, se houver (o componente comum do feather-ui).
    fn dialogo_formatar_controle(&self) -> Option<feather_ui::dialogo::Dialogo<'_, Msg>> {
        let r = self.objetos.formatar_controle.as_ref()?;
        let tipo = r.tipo?;
        let mut abas_ui = row![].spacing(4);
        for (aba, rotulo) in abas(tipo) {
            abas_ui = abas_ui.push(button(text(tr!(rotulo)).size(13)).style(estilo_botao_pequeno(aba == r.aba)).padding([7, 12])
                .on_press(ev(Evento::ControleAba(aba))));
        }
        let com_texto = matches!(tipo, T::Botao | T::Rotulo | T::Caixa | T::Opcao | T::Grupo);
        let conteudo: Element<'_, Msg> = match r.aba {
            AbaControle::Controle => self.aba_controle(r, tipo),
            AbaControle::Tamanho => column![
                linha("Altura (cm):", &r.altura, Campo::Altura, true),
                linha("Largura (cm):", &r.largura, Campo::Largura, true),
            ].spacing(9).into(),
            AbaControle::Protecao => column![
                marcar("Bloqueado", r.bloqueado, Opcao::Bloqueado),
            ].push_maybe(com_texto.then(|| marcar("Bloquear texto", r.bloquear_texto, Opcao::BloquearTexto)))
                .push(nota("Bloquear objetos só tem efeito quando a planilha está protegida."))
                .spacing(12).into(),
            AbaControle::Propriedades => column![
                text(tr!("Posicionamento do objeto")).size(13),
                column![
                    escolha(&[(0u8, "Mover e redimensionar com células")], r.posicao, Evento::ControlePosicao),
                    escolha(&[(1u8, "Mover, mas não dimensionar com células")], r.posicao, Evento::ControlePosicao),
                    escolha(&[(2u8, "Não mover nem dimensionar com células")], r.posicao, Evento::ControlePosicao),
                ].spacing(5),
                nota("O FeatherSheet guarda essa escolha no arquivo; na tela o controle ainda acompanha as células."),
                marcar("Imprimir objeto", r.imprimir, Opcao::Imprimir),
            ].spacing(12).into(),
            AbaControle::Fonte => column![
                linha("Tamanho (pt):", &r.fonte_tam, Campo::FonteTam, true),
                row![marcar("Negrito", r.negrito, Opcao::Negrito), marcar("Itálico", r.italico, Opcao::Italico)].spacing(20),
                text(tr!("Cor")).size(13),
                paleta(AlvoCor::Texto, r.cor_texto.as_deref(), &CORES_TEXTO),
            ].spacing(12).into(),
            AbaControle::Alinhamento => {
                use modelo::{AlinharH as H, AlinharV as V};
                column![
                    text(tr!("Horizontal")).size(13),
                    escolha(&[(H::Esquerda, "Esquerda"), (H::Centro, "Centro"), (H::Direita, "Direita")], r.alinhar.h, Evento::ControleAlinharH),
                    text(tr!("Vertical")).size(13),
                    escolha(&[(V::Topo, "Superior"), (V::Meio, "Centro"), (V::Baixo, "Inferior")], r.alinhar.v, Evento::ControleAlinharV),
                ].spacing(10).into()
            }
            AbaControle::CoresELinhas => column![
                text(tr!("Preenchimento")).size(13),
                paleta(AlvoCor::Fundo, r.fundo.as_deref(), &CORES_FUNDO),
                text(tr!("Linha")).size(13),
                paleta(AlvoCor::Linha, r.linha.as_deref(), &CORES_LINHA),
            ].spacing(10).into(),
            AbaControle::TextoAlt => column![
                text(tr!("Texto alternativo (lido por leitores de tela):")).size(13),
                text_input("", &r.alt).size(13).padding([6, 8]).on_input(|t| ev(Evento::ControleCampo(Campo::Alt, t))),
            ].spacing(9).into(),
        };
        let corpo = column![pelicano::scrollable(abas_ui).direction(pelicano::Direction::Horizontal).scrollbar_width(3.0), container(conteudo).padding([6, 2])].spacing(12);
        Some(self.dialogo(tr!("Formatar controle"), corpo)
            .icone("forms")
            .largura(480.0)
            .ao_fechar(ev(Evento::ControleCancelar))
            .secundario(tr!("Cancelar"), Some(ev(Evento::ControleCancelar)))
            .primario("OK", Some(ev(Evento::ControleOk))))
    }

    /// Põe o diálogo sobre a tela, se estiver aberto.
    pub(super) fn sobrepor_formatar_controle<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        match self.dialogo_formatar_controle() { Some(d) => d.sobre(base), None => base }
    }

    /// Abre o diálogo para o controle `id` da aba.
    pub(super) fn abrir_formatar_controle(&mut self, id: u64) {
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        let regua = super::ReguaAba(&aba.sheet);
        let r = self.objetos_da_aba().and_then(|(_, l)| l.obter(id).and_then(|o| {
            let ret = o.retangulo(&regua);
            let c = o.controle.as_ref()?;
            let mut r = Rascunho::de(o, c, (ret.largura, ret.altura));
            // O valor de verdade é o da célula ligada (o guardado no controle pode estar velho).
            match (c.tipo, super::controles::numero_ligado(self, c)) {
                (T::Girar | T::Rolagem, Some(n)) => r.valor = (n.round() as i64).to_string(),
                (T::Caixa, Some(n)) => r.marcado = u8::from(n != 0.0),
                _ => {}
            }
            (r.valor_original, r.marcado_original) = (r.valor.clone(), r.marcado);
            Some(r)
        }));
        self.objetos.formatar_controle = r;
    }

    /// OK do diálogo: o controle novo vai para o arquivo e o valor atual para a célula ligada.
    pub(super) fn confirmar_formatar_controle(&mut self) {
        let Some(r) = self.objetos.formatar_controle.clone() else { return };
        let Some((_, lidos)) = self.objetos_da_aba() else { return };
        let Some(c) = lidos.obter(r.id).and_then(|o| o.controle.clone()) else { self.objetos.formatar_controle = None; return };
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        let regua = super::ReguaAba(&aba.sheet);
        let fonte_atual = lidos.obter(r.id).and_then(|o| o.fonte.clone());
        match r.aplicar(&c).and_then(|novo| Ok((novo, r.tamanho()?, r.fonte(fonte_atual)?))) {
            Err(e) => self.status = format!("{}: {e}", tr!("Formatar controle")),
            Ok((novo, (w, h), fonte)) => {
                let mut objetos = (*lidos).clone();
                if let Some(o) = objetos.objetos.iter_mut().find(|o| o.id == r.id) {
                    o.controle = Some(novo.clone());
                    (o.fonte, o.alinhar) = (fonte, r.alinhar);
                    let ret = o.retangulo(&regua);
                    // Só re-ancora se mudou de verdade (o cm arredondado não pode mexer no controle).
                    if (ret.largura - w).abs() > 0.5 || (ret.altura - h).abs() > 0.5 {
                        o.ancorar(Retangulo::novo(ret.x, ret.y, w, h), &regua);
                    }
                }
                let opcoes = super::controles::opcoes_da_aba(&objetos);
                // Só o que mudou no diálogo vai para a célula ligada (senão uma fórmula ali seria trocada).
                let para_celula = match novo.tipo {
                    T::Girar | T::Rolagem if r.valor != r.valor_original => novo.valor,
                    T::Caixa if r.marcado != r.marcado_original => Some(i64::from(r.marcado)),
                    T::Opcao if r.marcado == 1 && r.marcado_original != 1 =>
                        opcoes.iter().position(|(i, _)| *i == r.id).map(|p| p as i64 + 1),
                    _ => None,
                };
                match para_celula {
                    // Grava célula e controle juntos (e desmarca as outras opções do grupo).
                    Some(v) => self.gravar_valor_do_controle(&objetos, r.id, &novo, v),
                    None => { self.gravar_objetos(&objetos, tr!("Formatar controle").into()); }
                }
                self.objetos.formatar_controle = None;
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ok_confere_limites_e_prende_o_valor() {
        let mut c = ControleFormulario::novo(T::Girar);
        c.valor = Some(5);
        let o = ObjetoSolto::novo_controle(c.clone(), Default::default(), Default::default());
        let mut r = Rascunho::de(&o, &c, (20.0, 40.0));
        r.campo(Campo::Min, "1".into());
        r.campo(Campo::Max, "10".into());
        r.campo(Campo::Valor, "50".into());
        r.campo(Campo::Celula, "$M$8".into());
        let novo = r.aplicar(&c).unwrap();
        assert_eq!((novo.min, novo.max, novo.valor, novo.celula.as_deref()), (Some(1), Some(10), Some(10), Some("$M$8")));
        r.campo(Campo::Max, "40000".into());
        assert!(r.aplicar(&c).is_err(), "acima de 30000");
        r.campo(Campo::Max, "0".into());
        assert!(r.aplicar(&c).is_err(), "mínimo maior que o máximo");
        // Botão de girar ignora a mudança de página.
        r.campo(Campo::Max, "10".into());
        r.campo(Campo::Pagina, "7".into());
        assert_eq!(r.aplicar(&c).unwrap().pagina, None);
        // Tamanho em cm, com vírgula; Proteção e 3D vão para o controle.
        assert_eq!(r.largura, "0,53");
        r.campo(Campo::Altura, "2,54".into());
        assert!((r.tamanho().unwrap().1 - 96.0).abs() < 0.01);
        r.campo(Campo::Altura, "abc".into());
        assert!(r.tamanho().is_err());
        r.opcao(Opcao::Bloqueado, false);
        r.opcao(Opcao::Sombreado, false);
        let novo = r.aplicar(&c).unwrap();
        assert!(novo.desbloqueado && novo.sem_3d && !novo.nao_imprimir);
        r.cor(AlvoCor::Fundo, Some("#ffff99".into()));
        r.posicao = 2;
        let novo = r.aplicar(&c).unwrap();
        assert_eq!((novo.fundo.as_deref(), novo.posicao), (Some("#ffff99"), 2));
        // Fonte: vírgula aceita, fora de 4–96 recusa, sem nada volta a padrão.
        r.campo(Campo::FonteTam, "12,5".into());
        r.opcao(Opcao::Negrito, true);
        let f = r.fonte(None).unwrap().unwrap();
        assert_eq!((f.tamanho, f.negrito), (Some(12.5), true));
        r.campo(Campo::FonteTam, "200".into());
        assert!(r.fonte(None).is_err());
        r.campo(Campo::FonteTam, "".into());
        r.opcao(Opcao::Negrito, false);
        assert_eq!(r.fonte(None).unwrap(), None);
        // Mínimo sem máximo acima do padrão (100): não pode derrubar o programa.
        let mut r2 = Rascunho::de(&o, &c, (20.0, 40.0));
        r2.campo(Campo::Min, "200".into());
        r2.campo(Campo::Max, "".into());
        r2.campo(Campo::Valor, "250".into());
        assert_eq!(r2.aplicar(&c).unwrap().valor, Some(200));
        assert_eq!(abas(T::Botao)[0].0, AbaControle::Fonte);
        assert!(!abas(T::Botao).iter().any(|(a, _)| *a == AbaControle::Controle));
    }
}
