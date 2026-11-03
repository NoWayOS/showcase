// The Screen Compiler gallery, 11 of 11: the style properties a screen may declare: sizes, alignment, and
// the theme's colours by name - never a colour value, so the
// large-text, high-contrast theme recolours them.
// Each example is captioned with the TSX that made it. Written for the showcase in TSX and
// converted at build time like every other screen here; the launcher's Screen Compiler
// tile opens the first, and Back and Next go round them all.
import { Button, Card, Column, HeaderBar, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 8;
const EDGE = 16;
const TIGHT = 8;

type GalleryProps = {
  onHome: () => void;
  onBack: () => void;
  onNext: () => void;
};

export function GalleryCssScreen(props: GalleryProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <HeaderBar gap={GAP} padding={TIGHT}>
          <Button id="home" variant="secondary" onPress={props.onHome}>
            Home
          </Button>
          <Text id="title" role="heading">
            Declared CSS
          </Text>
          <Spacer />
          <Text>
            11 of 11
          </Text>
          <Button variant="secondary" onPress={props.onBack}>
            Back
          </Button>
          <Button id="next" onPress={props.onNext}>
            Next
          </Button>
        </HeaderBar>
        <Row gap={EDGE}>
          <Card
            style={{
              width: 280,
              height: 120,
              gap: 4,
              padding: 12,
              alignItems: "center",
              justifyContent: "center",
              backgroundColor: "input",
              borderColor: "focus",
              borderWidth: 3,
              borderRadius: 24,
            }}
          >
            <Text style={{ color: "text-muted", textAlign: "center" }}>Centred, muted</Text>
            <Text style={{ color: "error", textAlign: "center" }}>In the error colour</Text>
          </Card>
          <Column gap={4}>
            <Text role="caption">{'<Card style={{ width: 280, height: 120,'}</Text>
            <Text role="caption">{'  alignItems: "center", justifyContent: "center",'}</Text>
            <Text role="caption">{'  backgroundColor: "input", borderColor: "focus",'}</Text>
            <Text role="caption">{'  borderWidth: 3, borderRadius: 24 }}>'}</Text>
            <Text role="caption">{'<Text style={{ color: "text-muted", textAlign: "center" }}>'}</Text>
          </Column>
        </Row>
        <Row style={{ minWidth: 320, maxWidth: 560, justifyContent: "space-between" }}>
          <Text>Spread</Text>
          <Text>between</Text>
          <Text>320 and 560</Text>
        </Row>
        <Text role="caption">{'<Row style={{ minWidth: 320, maxWidth: 560, justifyContent: "space-between" }}>'}</Text>
      </Column>
    </Screen>
  );
}
