//! Porta setBorder/setOutline de src/macros/api-formato.ts.
//! Estilos de borda são combinados por célula sem remover outras bordas,
//! com os mesmos limites explícitos de 20 mil células do legado.
use std::collections::BTreeMap;
use serde_json::Value as Json;
use feathersheet_core::{
    celulas::{
        estilo_parcial::{Campo, EstiloParcial, mesclar},
        refs::Rect,
    },
    planilha::{sheet::{edicao::Snap, Sheet}},
};
use crate::operacoes_formato;

const TETO: u64 = 20_000;

#[derive(Debug, Clone)]
pub(crate) struct Opcoes {
    top: bool, bottom: bool, left: bool, right: bool,
    inside_h: bool, inside_v: bool, all: bool, none: bool,
    color: Option<String>,
}
fn sim(o: &Json, chave: &str) -> Result<bool, String> {
    let v=&o[chave];
    if v.is_null() { Ok(false) }
    else { v.as_bool().ok_or_else(||feathersheet_core::trf!("Opção {chave} deve ser verdadeiro/falso")) }
}
pub(crate) fn preparar(area: &Rect, op: &str, opts: &Json) -> Result<Opcoes, String> {
    if area.area() > TETO {
        return Err(feathersheet_core::tr!("Bordas não aceitam mais de 20.000 células por chamada").into());
    }
    if !opts.is_object() && !opts.is_null() {
        return Err(feathersheet_core::tr!("As opções de borda devem ser um objeto").into());
    }
    if let Some(style)=opts["style"].as_str() {
        if style!="solid" { return Err(feathersheet_core::tr!("Estilo de borda suportado: solid").into()); }
    } else if !opts["style"].is_null() {
        return Err(feathersheet_core::tr!("Estilo de borda deve ser texto").into());
    }
    if !opts["width"].is_null() && opts["width"].as_f64()!=Some(1.0) {
        return Err(feathersheet_core::tr!("Espessura de borda suportada: 1").into());
    }
    if sim(opts,"diagonalDown")? || sim(opts,"diagonalUp")? {
        return Err(feathersheet_core::tr!("Bordas diagonais ainda não são suportadas").into());
    }
    let cor = if opts["color"].is_null() {None}
        else {Some(operacoes_formato::cor(&opts["color"])? )};
    let contorno = op=="setOutline";
    Ok(Opcoes {
        top:contorno || sim(opts,"top")?,
        bottom:contorno || sim(opts,"bottom")?,
        left:contorno || sim(opts,"left")?,
        right:contorno || sim(opts,"right")?,
        inside_h:sim(opts,"insideH")?,
        inside_v:sim(opts,"insideV")?,
        all:sim(opts,"all")?,
        none:sim(opts,"none")?,
        color:cor,
    })
}

fn anterior(estilo: Option<&EstiloParcial>) -> BTreeMap<String, Campo> {
    match estilo.and_then(|x|x.get("borders")) {
        Some(Campo::Objeto(b)) => b.clone(),
        _ => BTreeMap::new(),
    }
}
fn ativo(b:&BTreeMap<String, Campo>) -> bool {
    ["top","right","bottom","left"].iter()
        .any(|k|matches!(b.get(*k),Some(Campo::Logico(true))))
}
/// Preserva a herança de outras propriedades e modifica somente a borda.
/// Já recebe a transação aberta pelo host.
pub(crate) fn aplicar(sheet:&mut Sheet, area:&Rect, opts:&Opcoes) -> usize {
    let mut alteradas = 0usize;
    for r in area.r0..=area.r1 {
        for c in area.c0..=area.c1 {
            let foto = sheet.snapshot(r,c);
            let atual = sheet.estilo_mesclado(r,c);
            let era = anterior(atual.as_ref());
            let mut borda=if opts.none {BTreeMap::new()} else {era.clone()};
            let mut ligar=|nome:&str| {borda.insert(nome.into(), Campo::Logico(true));};
            if opts.all {
                for lado in ["top","right","bottom","left"] {ligar(lado);}
            }
            if opts.top && r==area.r0 {ligar("top");}
            if opts.bottom && r==area.r1 {ligar("bottom");}
            if opts.left && c==area.c0 {ligar("left");}
            if opts.right && c==area.c1 {ligar("right");}
            if opts.inside_h && r<area.r1 {ligar("bottom");}
            if opts.inside_v && c<area.c1 {ligar("right");}
            if let Some(cor)=&opts.color {
                if ativo(&borda) {borda.insert("color".into(),Campo::Texto(cor.clone()));}
            }
            if borda==era {continue;}
            let mut patch=EstiloParcial::new();
            if borda.is_empty() {patch.insert("borders".into(),Campo::Nulo);}
            else {patch.insert("borders".into(),Campo::Objeto(borda));}
            let direto = foto.as_ref().and_then(|f|f.style.as_ref());
            let novo = mesclar(direto,&patch);
            let bruto=foto.as_ref().map(|f|f.raw.clone()).unwrap_or_default();
            let next = if novo.is_empty() && bruto.is_empty() {None}
                else {Some(Snap{raw:bruto,style:(!novo.is_empty()).then_some(novo)})};
            if sheet.apply(r,c,next,true) {alteradas+=1;}
        }
    }
    alteradas
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn bordas_se_somam_e_contorno_preserva_conteudo() {
        let mut sheet=Sheet::new("A");
        sheet.set_cell(0,0,"42");
        let a=Rect::new(0,0,1,1);
        sheet.abrir_transacao();
        let topo=preparar(&a,"setBorder",&serde_json::json!({"top":true})).unwrap();
        assert_eq!(aplicar(&mut sheet,&a,&topo),2);
        let lado=preparar(&a,"setBorder",&serde_json::json!({"left":true})).unwrap();
        assert_eq!(aplicar(&mut sheet,&a,&lado),2);
        let centro=sheet.estilo_resolvido(0,0).unwrap().borders.unwrap();
        assert!(centro.top && centro.left);
        assert!(!centro.right);
        let p=preparar(&a,"setOutline",&serde_json::json!({"color":"#f00"})).unwrap();
        aplicar(&mut sheet,&a,&p);
        sheet.fechar_transacao();
        let b=sheet.estilo_resolvido(0,0).unwrap().borders.unwrap();
        assert_eq!(b.color,Some("#ff0000".into()));
        assert_eq!(sheet.get_raw(0,0),"42");
        assert!(sheet.undo());
        assert!(sheet.estilo_resolvido(0,0).is_none());
    }
    #[test]
    fn rejeita_opcoes_nao_suportadas_e_teto() {
        let a=Rect::cell(0,0);
        assert!(preparar(&a,"setBorder",&serde_json::json!({"style":"dashed"})).is_err());
        assert!(preparar(&a,"setBorder",&serde_json::json!({"width":2})).is_err());
        assert!(preparar(&a,"setBorder",&serde_json::json!({"diagonalUp":true})).is_err());
        assert!(preparar(&Rect::new(0,0,20_000,0),"setOutline",&Json::Null).is_err());
    }
}
