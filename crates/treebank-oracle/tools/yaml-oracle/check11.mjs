// The older YAML version, 1.1, through the same parser.
//
// A separate script rather than an argument because the shared oracle
// driver passes no arguments after a script path, and py-oracle already
// answers the same question the same way with check.py / check2.py.
import { judge } from "./judge.mjs";

judge("1.1");
