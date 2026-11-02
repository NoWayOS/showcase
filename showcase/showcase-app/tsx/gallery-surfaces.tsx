// The Screen Compiler gallery, 5 of 11: a card, a notice in each tone and a header bar: columns and rows
// drawn on a surface.
// Each example is captioned with the TSX that made it. Written for the showcase in TSX and
// converted at build time like every other screen here; the launcher's Screen Compiler
// tile opens the first, and Back and Next go round them all.
import { Button, Card, Column, HeaderBar, Notice, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 8;
const EDGE = 16;
const TIGHT = 8;

type GalleryProps = {
  onHome: () => void;
  onBack: () => void;
  onNext: () => void;
};

export function GallerySurfacesScreen(props: GalleryProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <HeaderBar gap={GAP} padding={TIGHT}>
          <Button id="home" variant="secondary" onPress={props.onHome}>
            Home
          </Button>
          <Text id="title" role="heading">
            Surfaces
          </Text>
          <Spacer />
          <Text>
            5 of 11
          </Text>
          <Button variant="secondary" onPress={props.onBack}>
            Back
          </Button>
          <Button id="next" onPress={props.onNext}>
            Next
          </Button>
        </HeaderBar>
        <Text role="caption">{'The bar above is a <HeaderBar>: its children side by side'}</Text>
        <Card gap={4} padding={TIGHT}>
          <Text weight="bold">A card</Text>
          <Text>Its children stacked on a white card</Text>
        </Card>
        <Text role="caption">{'<Card gap={4} padding={8}>'}</Text>
        <Notice tone="success" gap={4} padding={TIGHT}>
          <Text>Done: a success notice</Text>
        </Notice>
        <Notice tone="caution" padding={TIGHT}>
          <Text>Note this: a caution notice</Text>
        </Notice>
        <Notice tone="warning" padding={TIGHT}>
          <Text>Take care: a warning notice</Text>
        </Notice>
        <Text role="caption">{'<Notice tone="success" gap={4} padding={8}>, tone="caution" and tone="warning"'}</Text>
      </Column>
    </Screen>
  );
}
