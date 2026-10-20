---
shell: bash
---

## Bash-colored commands

Language tags affect styling, not execution.

```bash markmason
mm-test-cli echo "first"
mm-test-cli echo "second"
```

```expected
first
second
```

## Python-colored commands

```python markmason
mm-test-cli cat <<'PY'
name = "Markmason"
print(f"Hello, {name}!")
PY
```

```expected
name = "Markmason"
print(f"Hello, {name}!")
```

## C#-colored commands

```csharp markmason
mm-test-cli cat <<'CS'
var message = "Hello from C#";
Console.WriteLine(message);
CS
```

```expected
var message = "Hello from C#";
Console.WriteLine(message);
```
