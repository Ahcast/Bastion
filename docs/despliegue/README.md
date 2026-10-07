# CI/CD y despliegue

Fuente: [Documento maestro v0.5](../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

# 37. CI/CD

GitHub flow:

```text
feature branch
  ↓
commit
  ↓
tests
  ↓
pull request
  ↓
review
  ↓
merge
  ↓
deploy
```

Branching sugerido:

- `main`;
- feature branches.

Evitar complejidad innecesaria con `develop` si el equipo es pequeño.

---


# 38. Despliegue

## 38.1 Lightsail

GitHub Actions o deploy manual controlado:

- SSH;
- pull;
- venv;
- dependencies;
- restart scheduler/service.

## 38.2 Bluehost

Mecanismo a validar:

- SSH;
- SFTP;
- deploy nativo;
- Git support.

Debe quedar definido por senior según capacidades reales del plan.

---

