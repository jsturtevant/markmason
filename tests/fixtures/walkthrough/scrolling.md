# A complete scrollable section

Keep the command context visible, and scroll to inspect every output line.

```markmason
mm-test-cli echo started
mm-test-cli lines output-01 output-02 output-03 output-04 output-05 output-06 output-07 output-08 output-09 output-10 output-11 output-12 output-13 output-14 output-15 output-16 output-17 output-18 output-19 output-20 output-21 output-22 output-23 output-24 output-25 output-26 output-27 output-28 output-29 output-30
mm-test-cli echo finished
```

```expected-contains
started
finished
```
