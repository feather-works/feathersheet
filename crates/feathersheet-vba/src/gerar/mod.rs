//! O gerador: do projeto VBA às macros em JavaScript do FeatherSheet.
//!
//! - Cada `Sub` sem parâmetro obrigatório vira **uma macro** (é o que um botão chama). O
//!   runtime roda o módulo JS inteiro, isolado: por isso a macro leva junto todo
//!   procedimento que ela chama, e termina chamando a `Sub` de entrada.
//! - `Workbook_Open`, `Worksheet_Change` e `Worksheet_Activate` viram **eventos** (*Ao abrir*,
//!   *Ao mudar células*, *Ao ativar*).
//! - As `Function` públicas viram uma macro que as registra como **função de planilha**
//!   (`sheet.defineFunction`), para a célula que usava `=Extenso_Valor(A1)` continuar valendo.

mod ajuda;
mod contexto;
mod escrita;
mod excel;
mod expr;
mod funcoes;
mod instr;
mod tipos;

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;

pub use contexto::Pendencia;
use contexto::{js_nome, tipo_declarado, valor_inicial, Escopo, Gerador, InfoModulo, Macro, Programa, RefProc, Var};
use tipos::{Tipo, T};

use crate::ovba::TipoDeModulo;
use crate::sintaxe::{analisar, Expr, Instr, Proc, TipoInstr, TipoProc};
use crate::xlsm::ProjetoVba;

/// Uma macro pronta para o editor (Alt+F11).
#[derive(Clone, Debug, Serialize)]
pub struct MacroGerada {
    /// O nome da macro (o da `Sub`; com o módulo na frente se dois módulos repetem o nome).
    pub nome: String,
    pub modulo: String,
    pub codigo: String,
}

/// Um evento pronto (`aoAbrir`, `aoMudar`, `aoAtivar`).
#[derive(Clone, Debug, Serialize)]
pub struct EventoGerado {
    /// A aba (no evento de aba); `None` é o da pasta.
    pub aba: Option<String>,
    pub evento: &'static str,
    pub origem: String,
    pub codigo: String,
}

/// Um botão do Excel e a macro que ele chamava (`.OnAction`).
#[derive(Clone, Debug, Serialize)]
pub struct Botao {
    pub forma: String,
    pub macro_: String,
}

/// Um formulário (UserForm) convertido: o desenho e o que cada controle roda.
#[derive(Clone, Debug, Default)]
pub struct FormularioConvertido {
    pub desenho: crate::formulario::FormularioVba,
    /// (controle, função JS do módulo do formulário): o botão roda no clique; o campo,
    /// quando muda. A macro do controle é `<formulário>.<função>`.
    pub macros: Vec<(String, String)>,
}

/// O resultado da conversão.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Conversao {
    pub macros: Vec<MacroGerada>,
    /// Os formulários, cada um com o seu módulo em `macros` (o nome do módulo é o do formulário).
    #[serde(skip)]
    pub formularios: Vec<FormularioConvertido>,
    pub eventos: Vec<EventoGerado>,
    pub botoes: Vec<Botao>,
    /// O que não foi convertido (e os avisos), uma vez cada.
    pub pendencias: Vec<Pendencia>,
    pub linhas_vba: usize,
    pub instrucoes_convertidas: usize,
}

/// Os eventos que existem no FeatherSheet: (procedimento em minúsculas, tipo de módulo, evento).
const EVENTOS: [(&str, TipoDeModulo, &str); 3] = [("workbook_open", TipoDeModulo::Pasta, "aoAbrir"), ("worksheet_change", TipoDeModulo::Aba, "aoMudar"), ("worksheet_activate", TipoDeModulo::Aba, "aoAtivar")];

fn montar_programa(projeto: &ProjetoVba) -> Programa {
    let mut codenames = HashMap::new();
    let modulos = projeto
        .modulos
        .iter()
        .map(|m| {
            let aba = (m.tipo == TipoDeModulo::Aba).then(|| projeto.abas.iter().find(|(c, _)| c.eq_ignore_ascii_case(&m.nome)).map(|(_, n)| n.clone())).flatten();
            if let Some(a) = &aba {
                codenames.insert(m.nome.to_lowercase(), (js_nome(&m.nome), a.clone()));
            }
            InfoModulo { nome: m.nome.clone(), tipo: m.tipo, linhas: m.codigo.lines().map(String::from).collect(), ast: analisar(&m.codigo), aba }
        })
        .collect();
    // A aba cujo módulo não veio (VBA colado sem o módulo da aba) também é conhecida.
    for (c, nome) in &projeto.abas {
        codenames.entry(c.to_lowercase()).or_insert_with(|| (js_nome(c), nome.clone()));
    }
    let formularios = projeto.formularios.iter()
        .map(|f| (f.nome.to_lowercase(), (f.nome.clone(), f.controles.iter().map(|c| c.nome.clone()).collect())))
        .collect();
    Programa { modulos, codenames, formularios }
}

/// O que cada procedimento rendeu na primeira vez que foi escrito (para o total não contar
/// duas vezes o que entra em várias macros).
#[derive(Default)]
struct Totais {
    feitos: HashSet<RefProc>,
    pendencias: Vec<Pendencia>,
    convertidas: usize,
}

/// Escreve um procedimento como `function`.
fn escrever_proc(p: &Programa, m: &mut Macro, r: RefProc) -> Vec<String> {
    let proc = p.proc(r);
    let info = &p.modulos[r.0];
    let (atual, atual_nome) = match &info.aba {
        Some(nome) => (js_nome(&info.nome), Some(nome.clone())),
        None => ("sheet".to_string(), None),
    };
    if info.aba.is_some() {
        m.abas.insert(atual.clone(), atual_nome.clone().unwrap_or_default());
    }
    let mut g = Gerador { p, m, modulo: r.0, escopo: Escopo::default(), linhas: Vec::new(), nivel: 1, com: Vec::new(), lacos: Vec::new(), contador: 0, atual, atual_nome, rotulos: Vec::new() };
    let params = parametros(&mut g, proc);
    let funcao = matches!(proc.tipo, TipoProc::Funcao | TipoProc::PropriedadeLer);
    if funcao {
        g.escopo.funcao = Some(proc.nome.to_lowercase());
        g.escopo.vars.insert("$r".into(), Var { js: "$r".into(), tipo: tipo_declarado(proc.retorno.as_deref()), matriz: false, por_ref: false });
        if contexto::e_inteiro(proc.retorno.as_deref()) {
            g.escopo.inteiros.insert("$r".into());
        }
    }
    let n_params = g.escopo.ordem.len();
    g.coletar_declaracoes(&proc.corpo);
    g.corpo_do_proc(&proc.corpo);
    let nome = g.m.nomes.get(&r).cloned().unwrap_or_else(|| js_nome(&proc.nome));
    let mut saida = vec![format!("// {}.{} (linha {})", info.nome, proc.nome, proc.linha), format!("function {nome}({}) {{", params.join(", "))];
    saida.extend(conversoes_de_parametro(&mut g, proc));
    let locais: Vec<String> = g.escopo.ordem[n_params..].iter().filter_map(|k| g.escopo.vars.get(k)).map(|v| format!("{} = {}", v.js, valor_inicial(&v.tipo))).collect();
    if !locais.is_empty() {
        saida.push(format!("  let {};", locais.join(", ")));
    }
    if funcao {
        let t = g.escopo.vars.get("$r").map(|v| v.tipo.clone()).unwrap_or(Tipo::Valor(T::Var));
        saida.push(format!("  let $r = {};", valor_inicial(&t)));
    }
    saida.append(&mut g.linhas);
    if funcao {
        saida.push("  return $r;".into());
    }
    saida.push("}".into());
    saida
}

/// Os parâmetros (com o valor padrão do `Optional`), já registrados no escopo.
fn parametros(g: &mut Gerador, proc: &Proc) -> Vec<String> {
    proc.params
        .iter()
        .map(|pr| {
            let tipo = tipo_declarado(pr.tipo.as_deref());
            let js = g.declarar(&pr.nome, tipo.clone(), pr.matriz);
            if contexto::e_inteiro(pr.tipo.as_deref()) && !pr.matriz {
                g.escopo.inteiros.insert(js.clone());
            }
            if let Some(v) = g.escopo.vars.get_mut(&pr.nome.to_lowercase()) {
                v.por_ref = !pr.por_valor && matches!(tipo, Tipo::Valor(_));
            }
            match (&pr.padrao, pr.opcional) {
                (Some(e), _) => g.valor(e).map_or(js.clone(), |v| format!("{js} = {}", v.js)),
                (None, true) if !matches!(tipo, Tipo::Valor(T::Var)) => format!("{js} = {}", valor_inicial(&tipo)),
                _ => js,
            }
        })
        .collect()
}

/// `x = vba.str(x);` para os parâmetros com tipo: o VBA converte na entrada.
fn conversoes_de_parametro(g: &mut Gerador, proc: &Proc) -> Vec<String> {
    let mut v = Vec::new();
    for pr in &proc.params {
        let Some(var) = g.var(&pr.nome) else { continue };
        let ajuda = match var.tipo {
            Tipo::Valor(T::Num) if g.escopo.inteiros.contains(&var.js) => "cint",
            Tipo::Valor(T::Texto) => "str",
            Tipo::Valor(T::Num) => "n",
            Tipo::Valor(T::Bool) => "bool",
            _ => continue,
        };
        if var.matriz {
            continue;
        }
        let a = g.ajuda(ajuda);
        v.push(format!("  {0} = vba.{a}({0});", var.js));
    }
    v
}

/// Monta uma macro inteira: as funções pedidas a partir de `entrada`, o topo e o fim.
fn montar_macro(p: &Programa, totais: &mut Totais, entrada: RefProc, cabecalho: &str, fim: impl FnOnce(&mut Macro, &str) -> String) -> String {
    let mut m = Macro::default();
    let nome = m.pedir(p, entrada);
    let fim = fim(&mut m, &nome);
    let mut funcoes = Vec::new();
    let mut i = 0;
    while i < m.fila.len() {
        let r = m.fila[i];
        let (antes_pend, antes_conv) = (m.pendencias.len(), m.convertidas);
        funcoes.push(escrever_proc(p, &mut m, r).join("\n"));
        if totais.feitos.insert(r) {
            totais.pendencias.extend(m.pendencias[antes_pend..].iter().cloned());
            totais.convertidas += m.convertidas - antes_conv;
        }
        i += 1;
    }
    let mut topo = vec![cabecalho.to_string()];
    let ajudas = ajuda::bloco(&m.ajudas);
    if !ajudas.is_empty() {
        topo.push(ajudas);
    }
    if !m.abas.is_empty() {
        topo.push(m.abas.iter().map(|(var, nome)| format!("const {var} = wb.sheet({nome:?});")).collect::<Vec<_>>().join("\n"));
    }
    let globais = declarar_globais(p, &m);
    if !globais.is_empty() {
        topo.push(globais);
    }
    format!("{}\n\n{}\n\n{fim}\n", topo.join("\n\n"), funcoes.join("\n\n"))
}

/// As variáveis de módulo que a macro usa (`Public total As Long` → `let total = 0;`).
fn declarar_globais(p: &Programa, m: &Macro) -> String {
    let mut ordenadas: Vec<(&(usize, String), &String)> = m.globais.iter().collect();
    ordenadas.sort();
    let mut v = Vec::new();
    for ((mi, nome), js) in ordenadas {
        let ast = &p.modulos[*mi].ast;
        let Some(d) = ast.variaveis.iter().chain(&ast.constantes).find(|d| &d.nome.to_lowercase() == nome) else { continue };
        let inicial = match &d.valor {
            Some(Expr::Num(x)) => funcoes::num(*x),
            Some(Expr::Texto(s)) => funcoes::texto(s),
            Some(Expr::Bool(b)) => b.to_string(),
            Some(_) => "undefined /* valor da constante não convertido */".into(),
            None if d.dims.is_some() => "[]".into(),
            None => valor_inicial(&tipo_declarado(d.tipo.as_deref())).into(),
        };
        let kw = if d.valor.is_some() { "const" } else { "let" };
        v.push(format!("{kw} {js} = {inicial}; // variável do módulo {} (não guarda valor entre uma execução e outra)", p.modulos[*mi].nome));
    }
    v.join("\n")
}

fn cabecalho(origem: &str, arquivo: &str) -> String {
    format!(
        "// Convertida do VBA: {origem}{}.\n// Revise antes de usar: o que não tem equivalente está marcado com \"VBA não convertido\".",
        if arquivo.is_empty() { String::new() } else { format!(" ({arquivo})") }
    )
}

/// Os procedimentos que viram macro: `Sub` sem parâmetro obrigatório, fora dos eventos.
fn entradas(p: &Programa) -> Vec<RefProc> {
    let mut v = Vec::new();
    for (mi, m) in p.modulos.iter().enumerate() {
        for (pi, pr) in m.ast.procs.iter().enumerate() {
            let evento = pr.nome.to_lowercase().starts_with("workbook_") || pr.nome.to_lowercase().starts_with("worksheet_");
            if pr.tipo == TipoProc::Sub && !evento && pr.params.iter().all(|x| x.opcional) && m.tipo != TipoDeModulo::Classe {
                v.push((mi, pi));
            }
        }
    }
    v
}

fn converter_eventos(p: &Programa, totais: &mut Totais, arquivo: &str, c: &mut Conversao) {
    for (mi, m) in p.modulos.iter().enumerate() {
        for (pi, pr) in m.ast.procs.iter().enumerate() {
            let n = pr.nome.to_lowercase();
            let Some((_, _, evento)) = EVENTOS.iter().find(|(e, t, _)| *e == n && *t == m.tipo) else {
                if n.starts_with("workbook_") || n.starts_with("worksheet_") {
                    c.pendencias.push(Pendencia { modulo: m.nome.clone(), linha: pr.linha, vba: format!("Sub {}", pr.nome), motivo: "evento do Excel sem equivalente no FeatherSheet".into(), aviso: false });
                }
                continue;
            };
            let aba_js = js_nome(&m.nome);
            let alvo = if pr.params.is_empty() { String::new() } else { format!("valores.intervalo || {aba_js}.selection().address") };
            let codigo = montar_macro(p, totais, (mi, pi), &cabecalho(&format!("{}.{}", m.nome, pr.nome), arquivo), |_, nome| format!("{nome}({alvo});"));
            c.eventos.push(EventoGerado { aba: m.aba.clone(), evento, origem: format!("{}.{}", m.nome, pr.nome), codigo });
        }
    }
}

/// Eventos de controle que viram a macro do controle (`CommandButton1_Click`, `TextBox1_Change`).
const EVENTOS_DE_CONTROLE: [&str; 5] = ["click", "change", "afterupdate", "dblclick", "exit"];
/// Os do formulário que rodam ao abrir: dobrados no desenho quando são só valores fixos.
const AO_ABRIR: [&str; 2] = ["userform_initialize", "userform_activate"];

/// Cada formulário: o desenho, um módulo com o código dele e a macro de cada controle.
fn converter_formularios(p: &Programa, projeto: &ProjetoVba, totais: &mut Totais, arquivo: &str, c: &mut Conversao) {
    for form in &projeto.formularios {
        let mut desenho = form.clone();
        for ctl in &form.controles {
            if let crate::formulario::TipoVba::Outro(k) = ctl.tipo {
                c.pendencias.push(Pendencia { modulo: form.nome.clone(), linha: 0, vba: ctl.nome.clone(), motivo: format!("controle do formulário sem equivalente (tipo {k})"), aviso: false });
            }
        }
        let mut macros = Vec::new();
        if let Some(mi) = p.modulos.iter().position(|m| m.nome.eq_ignore_ascii_case(&form.nome)) {
            let modulo = &p.modulos[mi];
            let mut resto = Vec::new();
            for (pi, pr) in modulo.ast.procs.iter().enumerate() {
                if AO_ABRIR.contains(&pr.nome.to_lowercase().as_str()) {
                    dobrar_ao_abrir(p, mi, &pr.corpo, &mut desenho, &mut c.pendencias);
                } else {
                    resto.push((mi, pi));
                }
            }
            if let Some((&primeira, outras)) = resto.split_first() {
                let mut ligados = Vec::new();
                let mut avisos = Vec::new();
                let codigo = montar_macro(p, totais, primeira, &cabecalho(&format!("o formulário {}", form.nome), arquivo), |m, _| {
                    for &r in std::iter::once(&primeira).chain(outras) {
                        let js = m.pedir(p, r);
                        let nome = &p.proc(r).nome;
                        let ligado = nome.rsplit_once('_').filter(|(ctl, ev)| EVENTOS_DE_CONTROLE.contains(&ev.to_lowercase().as_str()) && p.controle(&form.nome, ctl).is_some());
                        match ligado {
                            Some((ctl, _)) => ligados.push((p.controle(&form.nome, ctl).unwrap_or(ctl).to_string(), js)),
                            None if nome.to_lowercase().starts_with("userform_") => avisos.push(Pendencia {
                                modulo: form.nome.clone(), linha: p.proc(r).linha, vba: format!("Sub {nome}"),
                                motivo: "evento do formulário sem equivalente (a função fica no módulo)".into(), aviso: true,
                            }),
                            None => {}
                        }
                    }
                    // Só as funções: cada controle chama a sua (`Formulário.função`).
                    String::new()
                });
                // Um controle com dois eventos (Click e Change) fica com o primeiro.
                ligados.dedup_by(|a, b| a.0 == b.0);
                c.pendencias.extend(avisos);
                macros = ligados;
                c.macros.push(MacroGerada { nome: form.nome.clone(), modulo: form.nome.clone(), codigo });
            }
        }
        c.formularios.push(FormularioConvertido { desenho, macros });
    }
}

/// `UserForm_Initialize` com valores fixos (`ComboBox1.AddItem "Sul"`, `TextBox1.Value = "0"`,
/// `Me.Caption = "Cadastro"`) vira o desenho; o resto, pendência.
fn dobrar_ao_abrir(p: &Programa, mi: usize, corpo: &[Instr], desenho: &mut crate::formulario::FormularioVba, pendencias: &mut Vec<Pendencia>) {
    let form = desenho.nome.clone();
    // O controle de `TextBox1`, `Me.TextBox1`, `UserForm1.TextBox1`.
    let controle = |e: &Expr| -> Option<String> {
        match e {
            Expr::Nome(n) => p.controle(&form, n).map(String::from),
            Expr::Membro(b, n) if matches!(&**b, Expr::Nome(x) if x.eq_ignore_ascii_case("me") || x.eq_ignore_ascii_case(&form)) => p.controle(&form, n).map(String::from),
            _ => None,
        }
    };
    let literal = |e: &Expr| match e {
        Expr::Texto(t) => Some(t.clone()),
        Expr::Num(n) => Some(funcoes::num(*n)),
        Expr::Bool(b) => Some(if *b { "1" } else { "0" }.into()),
        _ => None,
    };
    for i in corpo {
        let feito = match &i.k {
            TipoInstr::Chamar(Expr::Chamada(f, args)) => match (&**f, args.as_slice()) {
                (Expr::Membro(b, m), [a]) if m.eq_ignore_ascii_case("additem") => controle(b).zip(a.valor.as_ref().and_then(literal)).map(|(c, t)| {
                    desenho.controles.iter_mut().filter(|x| x.nome == c).for_each(|x| x.itens.push(t.clone()));
                }),
                _ => None,
            },
            TipoInstr::Atribuir { alvo, valor, .. } => {
                let (alvo_ctl, prop) = match alvo {
                    Expr::Membro(b, prop) if controle(b).is_some() => (controle(b), prop.to_lowercase()),
                    Expr::Membro(b, prop) if matches!(&**b, Expr::Nome(x) if x.eq_ignore_ascii_case("me")) && prop.eq_ignore_ascii_case("caption") => (None, "titulo".into()),
                    _ => (controle(alvo), "value".into()),
                };
                literal(valor).and_then(|v| match (alvo_ctl, prop.as_str()) {
                    (None, "titulo") => {
                        desenho.titulo = v;
                        Some(())
                    }
                    (Some(c), "value" | "text") => {
                        desenho.controles.iter_mut().filter(|x| x.nome == c).for_each(|x| x.valor = v.clone());
                        Some(())
                    }
                    (Some(c), "caption") => {
                        desenho.controles.iter_mut().filter(|x| x.nome == c).for_each(|x| x.texto = v.clone());
                        Some(())
                    }
                    _ => None,
                })
            }
            TipoInstr::Dim(_) | TipoInstr::Const(_) => Some(()),
            _ => None,
        };
        if feito.is_none() {
            pendencias.push(Pendencia {
                modulo: p.modulos[mi].nome.clone(), linha: i.linha, vba: p.fonte(mi, i).join("\n"),
                motivo: "UserForm_Initialize: só AddItem e valores fixos convertem (o formulário abre sem rodar código)".into(), aviso: false,
            });
        }
    }
}

/// A macro que registra as `Function` públicas dos módulos comuns como funções de planilha.
fn converter_funcoes(p: &Programa, totais: &mut Totais, arquivo: &str) -> Option<MacroGerada> {
    let funcoes: Vec<RefProc> = p
        .modulos
        .iter()
        .enumerate()
        .filter(|(_, m)| m.tipo == TipoDeModulo::Padrao)
        .flat_map(|(mi, m)| m.ast.procs.iter().enumerate().filter(|(_, pr)| pr.tipo == TipoProc::Funcao && pr.publico).map(move |(pi, _)| (mi, pi)))
        .collect();
    let (&primeira, resto) = funcoes.split_first()?;
    let resto = resto.to_vec();
    let codigo = montar_macro(p, totais, primeira, &cabecalho("as funções públicas (Function) dos módulos, para usar nas células", arquivo), |m, nome0| {
        let mut linhas = vec![format!("sheet.defineFunction({:?}, (...a) => {nome0}(...a));", p.proc(primeira).nome.to_uppercase())];
        for r in resto {
            let n = m.pedir(p, r);
            linhas.push(format!("sheet.defineFunction({:?}, (...a) => {n}(...a));", p.proc(r).nome.to_uppercase()));
        }
        linhas.join("\n")
    });
    Some(MacroGerada { nome: "Funções do VBA".into(), modulo: String::new(), codigo })
}

/// O nome da forma num `Shapes.Range(Array("Shape 3"))` / `Shapes("Botão 1")`.
fn forma_em(e: &Expr) -> Option<String> {
    match e {
        Expr::Chamada(f, args) => {
            let dentro_de_shapes = matches!(&**f, Expr::Membro(_, n) if n.eq_ignore_ascii_case("shapes") || n.eq_ignore_ascii_case("range"));
            let primeiro = args.first().and_then(|a| a.valor.as_ref());
            match primeiro {
                Some(Expr::Texto(t)) if dentro_de_shapes => Some(t.clone()),
                Some(Expr::Chamada(arr, xs)) if dentro_de_shapes && matches!(&**arr, Expr::Nome(n) if n.eq_ignore_ascii_case("array")) => xs.first().and_then(|a| match &a.valor {
                    Some(Expr::Texto(t)) => Some(t.clone()),
                    _ => None,
                }),
                _ => forma_em(f),
            }
        }
        Expr::Membro(b, _) => forma_em(b),
        _ => None,
    }
}

/// Os `.OnAction = "Macro"` (com a forma selecionada antes, quando dá para saber).
fn botoes(v: &[Instr], forma: &mut String, saida: &mut Vec<Botao>) {
    for i in v {
        match &i.k {
            TipoInstr::Chamar(e) => {
                if let Some(f) = forma_em(e) {
                    *forma = f;
                }
            }
            TipoInstr::Atribuir { alvo: Expr::Membro(b, n), valor: Expr::Texto(m), .. } if n.eq_ignore_ascii_case("onaction") => {
                let f = forma_em(b).unwrap_or_else(|| forma.clone());
                saida.push(Botao { forma: f, macro_: m.clone() });
            }
            TipoInstr::Se { ramos, senao } => {
                ramos.iter().for_each(|(_, c)| botoes(c, forma, saida));
                if let Some(s) = senao {
                    botoes(s, forma, saida);
                }
            }
            TipoInstr::Com { corpo, .. } | TipoInstr::Para { corpo, .. } | TipoInstr::Faca { corpo, .. } => botoes(corpo, forma, saida),
            _ => {}
        }
    }
}

/// Todas as funções `vba.*` num bloco só (para testar o JavaScript delas fora da macro).
#[doc(hidden)]
pub fn todas_as_ajudas() -> String {
    ajuda::todas()
}

/// Converte o projeto VBA. `arquivo` é só para o cabeçalho das macros.
pub fn converter(projeto: &ProjetoVba, arquivo: &str) -> Conversao {
    let p = montar_programa(projeto);
    let mut totais = Totais::default();
    let mut c = Conversao { linhas_vba: projeto.modulos.iter().map(|m| m.codigo.lines().filter(|l| !l.trim().is_empty() && !l.starts_with("Attribute ")).count()).sum(), ..Conversao::default() };
    let todas = entradas(&p);
    let mut repetidos: BTreeMap<String, usize> = BTreeMap::new();
    for r in &todas {
        *repetidos.entry(p.proc(*r).nome.to_lowercase()).or_default() += 1;
    }
    for r in todas {
        let pr = p.proc(r);
        let modulo = p.modulos[r.0].nome.clone();
        let nome = if repetidos[&pr.nome.to_lowercase()] > 1 { format!("{modulo}.{}", pr.nome) } else { pr.nome.clone() };
        let codigo = montar_macro(&p, &mut totais, r, &cabecalho(&format!("{modulo}.{}", pr.nome), arquivo), |_, n| format!("{n}();"));
        c.macros.push(MacroGerada { nome, modulo, codigo });
    }
    converter_eventos(&p, &mut totais, arquivo, &mut c);
    converter_formularios(&p, projeto, &mut totais, arquivo, &mut c);
    c.macros.extend(converter_funcoes(&p, &mut totais, arquivo));
    for m in &p.modulos {
        for pr in &m.ast.procs {
            botoes(&pr.corpo, &mut String::new(), &mut c.botoes);
        }
        for (linha, t) in &m.ast.ignorados {
            c.pendencias.push(Pendencia { modulo: m.nome.clone(), linha: *linha, vba: t.clone(), motivo: "trecho do módulo fora de procedimento".into(), aviso: false });
        }
    }
    c.pendencias.extend(totais.pendencias);
    c.pendencias.sort_by(|a, b| (&a.modulo, a.linha).cmp(&(&b.modulo, b.linha)));
    c.pendencias.dedup_by(|a, b| a.modulo == b.modulo && a.linha == b.linha && a.motivo == b.motivo);
    c.instrucoes_convertidas = totais.convertidas;
    c
}
