//! Geometria pura de intervalos da API TypeScript de macros.
//! O motor Rust já fornece Rect e parser A1. Não ler/escrever células.
use feathersheet_core::celulas::refs::{parse_range_ref, Rect, MAX_ROWS, MAX_COLS};
use serde_json::{json,Value as Json};

fn argumento_int(args:&Json,i:usize,op:&str)->Result<i64,String> {
    // Conta feita no JS (`k / 2 * 2`, `Math.round`) chega como 5.0, não 5.
    args[i].as_i64().or_else(||args[i].as_f64().filter(|x|x.fract()==0.0&&x.abs()<9e15).map(|x|x as i64))
        .ok_or_else(||format!("{op}: parâmetro deve ser inteiro"))
}
fn retangulo(args:&Json)->Result<Rect,String> {
    let s=args[1].as_str().ok_or("Informe um intervalo A1")?;
    let r=parse_range_ref(s)
        .ok_or_else(||format!("Intervalo inválido: {s}"))?.normalized();
    if r.sheet.is_some() {
        return Err("Use wb.sheet(nome).range(endereço) para outra aba".into());
    }
    Ok(r)
}
fn criar(r0:i64,c0:i64,r1:i64,c1:i64,op:&str)->Result<Rect,String> {
    if r0<0 || c0<0 || r1<r0 || c1<c0
        || r1>=i64::from(MAX_ROWS) || c1>=i64::from(MAX_COLS) {
        return Err(format!("{op}: intervalo fora dos limites da planilha"));
    }
    Ok(Rect::new(r0 as u32,c0 as u32,r1 as u32,c1 as u32))
}
/// Retorna string A1 para o proxy Boa, que cria MacroRange.
pub(crate) fn executar(op:&str,args:&Json)->Result<Json,String> {
    let area=retangulo(args)?;
    let saida=match op {
        "offset" => {
            let dl=argumento_int(args,2,op)?;
            let dc=argumento_int(args,3,op)?;
            let somar=|a:u32,b:i64|i64::from(a).checked_add(b)
                .ok_or_else(||"offset: deslocamento muito grande".to_string());
            criar(somar(area.r0,dl)?,somar(area.c0,dc)?,
                somar(area.r1,dl)?,somar(area.c1,dc)?,op)?
        }
        "resize" => {
            let linhas=argumento_int(args,2,op)?;
            let colunas=argumento_int(args,3,op)?;
            if linhas<1 || colunas<1 {
                return Err("resize: linhas e colunas devem ser >= 1".into());
            }
            let r1=i64::from(area.r0).checked_add(linhas-1)
                .ok_or("resize: linhas demais")?;
            let c1=i64::from(area.c0).checked_add(colunas-1)
                .ok_or("resize: colunas demais")?;
            criar(i64::from(area.r0),i64::from(area.c0),r1,c1,op)?
        }
        "columnOf" => {
            let n=argumento_int(args,2,op)?;
            let total=i64::from(area.c1-area.c0)+1;
            if n<1 || n>total {
                return Err(format!("columnOf: há {total} coluna(s)"));
            }
            let c=area.c0+(n as u32)-1;
            Rect::new(area.r0,c,area.r1,c)
        }
        "rowOf" => {
            let n=argumento_int(args,2,op)?;
            let total=i64::from(area.r1-area.r0)+1;
            if n<1 || n>total {
                return Err(format!("rowOf: há {total} linha(s)"));
            }
            let r=area.r0+(n as u32)-1;
            Rect::new(r,area.c0,r,area.c1)
        }
        _ => return Err(format!("Operação não implementada: {op}")),
    };
    Ok(json!(crate::operacoes_valores::endereco_a1(&saida)))
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn deslocamento_e_redimensionamento() {
        assert_eq!(executar("offset",&json!([0,"B2:C3",2,-1])).unwrap(),json!("A4:B5"));
        assert_eq!(executar("resize",&json!([0,"C7:D9",2,3])).unwrap(),json!("C7:E8"));
        assert!(executar("offset",&json!([0,"A1",-1,0])).is_err());
        assert!(executar("resize",&json!([0,"XFD100000000",2,1])).is_err());
    }
    #[test]
    fn linha_coluna_e_limites_dez_milhoes() {
        assert_eq!(executar("columnOf",&json!([0,"B2:D5",2])).unwrap(),json!("C2:C5"));
        assert_eq!(executar("rowOf",&json!([0,"B2:D5",3])).unwrap(),json!("B4:D4"));
        assert_eq!(executar("offset",&json!([0,"C:C",0,1])).unwrap(),json!("D1:D100000000"));
        assert!(executar("rowOf",&json!([0,"A1:C3",4])).is_err());
    }
}
