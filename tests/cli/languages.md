---
shell: bash
---

# Language-tagged command blocks

Put the language first and `markmason` second. Standard Markdown renderers can
highlight the code, and `mm --step` colors its source without changing commands,
assertions, or execution mode. These examples pass recognizable source through
the test CLI with Bash heredocs, so no Python or C# runtime is needed.

## Bash styling

```bash markmason
mm-test-cli echo "bash"
```

```expected
bash
```

## Python styling

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

## C# styling

Use `csharp`, `c#`, or `cs`.

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

```c# markmason
mm-test-cli cat <<'CS'
Console.WriteLine("The c# alias uses C# highlighting.");
CS
```

```expected
Console.WriteLine("The c# alias uses C# highlighting.");
```

## Unknown languages remain readable

Unsupported language tags display plain source in the walkthrough.

```custom-language markmason
mm-test-cli echo "plain source"
```

```expected
plain source
```

## Expected failures still use the same metadata

```bash markmason (exit 2)
mm-test-cli fail
```

```expected
stdout
stderr
```

## Ordinary code fences still do not execute

The `markmason` marker is required, so these are just documentation.

```python
raise RuntimeError("must not execute")
```

```csharp
throw new Exception("must not execute");
```
