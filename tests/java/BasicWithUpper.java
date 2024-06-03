import static java.lang.StringTemplate.STR;

import java.nio.file.Files;
import java.nio.file.Paths;
import java.io.IOException;
import java.util.logging.*;


Logger LOGGER = Logger.getLogger("basic");

void main() throws IOException {
    System.setProperty("java.util.logging.SimpleFormatter.format",
                   "%1$tF %1$tT %4$s %2$s: %5$s%6$s%n");
    var fh = new FileHandler("basic.log");
    fh.setFormatter(new SimpleFormatter());
    LOGGER.addHandler(fh);
    LOGGER.setLevel(Level.FINE);
    LOGGER.fine("Hello from main");
    for (int i = 0; i < 3; i++) {
        foo(i);
    }
}

void foo(int i) {
    LOGGER.fine(STR."Hello from foo i=\{i}");
}
