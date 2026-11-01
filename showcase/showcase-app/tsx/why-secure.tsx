// The showcase's "Why is this secure?" panel, from the launcher's
// sixth tile: six short claims, each marked Built, Designed or Simulated,
// in words the maintainers have approved. The maintainer approves any
// change to the wording before it is published. No field, and
// nothing kept: Home goes back to the launcher. Its header is a bar in the
// launcher's brand colour.
import { Button, Card, Column, HeaderBar, Icon, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 8;
const GAP_ROW = 12;
const EDGE = 16;
const TIGHT = 8;

type WhySecureProps = {
  onHome: () => void;
};

export function WhySecureScreen({ onHome }: WhySecureProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <HeaderBar gap={GAP_ROW} padding={TIGHT}>
          <Button id="home" variant="secondary" onPress={onHome}>
            Home
          </Button>
          <Icon name="shield-check" />
          <Text id="title" role="title">
            Why is this secure?
          </Text>
        </HeaderBar>
        <Text role="caption">In development: a platform, not a certified product.</Text>
        <Card gap={GAP} padding={TIGHT}>
          <Row gap={GAP_ROW}>
            <Text role="heading">No browser</Text>
            <Text>React screens, compiled to native Rust</Text>
            <Spacer />
            <Text role="label" weight="bold">
              Built
            </Text>
          </Row>
        </Card>
        <Card gap={GAP} padding={TIGHT}>
          <Row gap={GAP_ROW}>
            <Text role="heading">Sealed compartments</Text>
            <Text>The screen, input and each kiosk run apart on seL4</Text>
            <Spacer />
            <Text role="label" weight="bold">
              Built
            </Text>
          </Row>
        </Card>
        <Card gap={GAP} padding={TIGHT}>
          <Row gap={GAP_ROW}>
            <Text role="heading">Contained</Text>
            <Text>Designed so a compromise of one part stays there</Text>
            <Spacer />
            <Text role="label" weight="bold">
              Designed
            </Text>
          </Row>
        </Card>
        <Card gap={GAP} padding={TIGHT}>
          <Row gap={GAP_ROW}>
            <Text role="heading">Wiped each session</Text>
            <Text>Answers wiped from memory when a session ends</Text>
            <Spacer />
            <Text role="label" weight="bold">
              Built
            </Text>
          </Row>
        </Card>
        <Card gap={GAP} padding={TIGHT}>
          <Row gap={GAP_ROW}>
            <Text role="heading">Card reader apart</Text>
            <Text>The pump screen never sees a card</Text>
            <Spacer />
            <Text role="label" weight="bold">
              Simulated
            </Text>
          </Row>
        </Card>
        <Card gap={GAP} padding={TIGHT}>
          <Row gap={GAP_ROW}>
            <Text role="heading">Signed updates</Text>
            <Text>Designed to be signed and to fall back safely</Text>
            <Spacer />
            <Text role="label" weight="bold">
              Designed
            </Text>
          </Row>
        </Card>
        <Spacer />
        <Text role="caption">Built: runs in this demo. Designed: planned, not built yet. Simulated: stands in for a real device.</Text>
      </Column>
    </Screen>
  );
}
