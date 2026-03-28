# evidence_backend

- `src`
  Rust 백엔드 코드
- `schema.sql`
  SQLite 스키마
- `eval`
  평가용 데이터
상위 설계 문서는 저장소 루트로 이동해 있다.

- `..\..\아키텍처_보고서.md`
- `..\..\실제_설계_문서.md`
- `..\..\파일_상세_정리.md`

실행 데이터는 코드 디렉토리 밖으로 분리되어 있다.

- SQLite DB: `C:\Users\Elise\Desktop\dev\data\backend\evidence.db`
- RAG 스테이징 매니페스트: `C:\Users\Elise\Desktop\dev\data\rag_stage\manifests\manifest.csv`

## 실행 예시

```powershell
cd C:\Users\Elise\Desktop\dev\code\evidence_backend
C:\Users\Elise\.cargo\bin\cargo.exe run -- serve --bind 127.0.0.1:8080
```

## 주요 엔드포인트
- `GET /health`
- `GET /api/summary`
- `GET /api/families`
- `GET /api/search?keyword=<text>&family=<family>&limit=20`
- `GET /api/rows/<row_id>`
- `GET /api/documents/<document_id>/rows?sheet_name=<sheet>&limit=50`
